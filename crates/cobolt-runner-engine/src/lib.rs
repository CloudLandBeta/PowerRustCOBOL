// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The one WebAssembly engine of a built Desktop program (DESKTOP-WASM-001 T13, plan §4.6).
//!
//! * [`config`] — the settings every engine is made with, for the generator's tests and for the
//!   runner alike: multi-memory, NaN canonicalisation, an explicit target at a fixed CPU level,
//!   parallel compilation.
//! * [`backend_config`] — the Backend runner's settings (spec 114 D3, D4): [`config`] plus epoch interruption, so a
//!   call that must end can be stopped from outside, and, with the `pooling` feature, an instance pool.
//! * [`precompile`] — the Build's machine code for a [`Target`]: x86-64-v2 on x86-64, the Apple
//!   M1's features on aarch64. It depends on the target only, never on the machine that builds.
//! * [`load`] — the runner's one way in, in a fixed order: the code's sha256, then the engine's
//!   compatibility hash, then the code itself; failing those, the local [`Cache`]; failing that,
//!   compile at load (and fill the cache). It names the [`LoadPath`] it took and, when the machine
//!   code was passed by, [`Skipped`] says why.

use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Instant, SystemTime};

use sha2::{Digest, Sha256};
use target_lexicon::{Architecture, Triple};
use wasmtime::{Engine, Module};

/// The local cache's size limit: 512 MiB, least recently used first out (plan §4.6).
pub const CACHE_LIMIT: u64 = 512 * 1024 * 1024;

/// x86-64-v2 in Cranelift's flags: SSE3, SSSE3, SSE4.1, SSE4.2, POPCNT and CMPXCHG16B.
const X86_64_V2: &[&str] = &["has_sse3", "has_ssse3", "has_sse41", "has_sse42", "has_popcnt", "has_cmpxchg16b"];
/// The Apple M1's level in Cranelift's aarch64 flags: LSE atomics, pointer authentication, FP16.
const APPLE_M1: &[&str] = &["has_lse", "has_pauth", "has_fp16"];

/// Where machine code runs: a target triple and the CPU features it may use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    triple: Triple,
    flags: Vec<&'static str>,
}

impl Target {
    /// `triple` at its architecture's baseline level: what the Build makes machine code for.
    pub fn baseline(triple: &str) -> Result<Self, String> {
        let triple = Triple::from_str(triple).map_err(|e| format!("target {triple}: {e}"))?;
        let flags = baseline_flags(&triple).to_vec();
        Ok(Self { triple, flags })
    }

    /// This machine: its triple at the baseline level, less any feature this CPU lacks. On every
    /// Apple silicon Mac and every x86-64-v2 PC it equals [`Target::baseline`] of the same triple,
    /// so the Build's machine code loads; an older CPU compiles at load instead of being refused.
    pub fn host() -> Self {
        let triple = target_lexicon::HOST;
        let flags = baseline_flags(&triple).iter().copied().filter(|f| host_has(f)).collect();
        Self { triple, flags }
    }

    /// The target triple, as `rustc` and wasmtime spell it.
    pub fn triple(&self) -> String {
        self.triple.to_string()
    }

    /// The Cranelift CPU flags this target turns on.
    pub fn flags(&self) -> &[&'static str] {
        &self.flags
    }
}

fn baseline_flags(triple: &Triple) -> &'static [&'static str] {
    match triple.architecture {
        Architecture::X86_64 => X86_64_V2,
        Architecture::Aarch64(_) => APPLE_M1,
        _ => &[],
    }
}

/// Whether this CPU has a baseline flag's feature, by the names wasmtime checks code against when it
/// loads it (`Engine::check_compatible_with_isa_flag`).
fn host_has(flag: &str) -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        match flag {
            "has_sse3" => std::arch::is_x86_feature_detected!("sse3"),
            "has_ssse3" => std::arch::is_x86_feature_detected!("ssse3"),
            "has_sse41" => std::arch::is_x86_feature_detected!("sse4.1"),
            "has_sse42" => std::arch::is_x86_feature_detected!("sse4.2"),
            "has_popcnt" => std::arch::is_x86_feature_detected!("popcnt"),
            "has_cmpxchg16b" => std::arch::is_x86_feature_detected!("cmpxchg16b"),
            _ => false,
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        match flag {
            "has_lse" => std::arch::is_aarch64_feature_detected!("lse"),
            "has_pauth" => std::arch::is_aarch64_feature_detected!("paca"),
            "has_fp16" => std::arch::is_aarch64_feature_detected!("fp16"),
            _ => false,
        }
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        let _ = flag;
        false
    }
}

/// The one engine configuration (plan §4.6). Multi-memory for the arena (WASM-017 R9); NaN
/// canonicalisation so `COMP-1`/`COMP-2` give the same bits on every machine (WASM-010 R13); an
/// explicit target, so no feature of the machine that compiles leaks into the code; machine code
/// compiled on every core.
pub fn config(target: &Target) -> Result<wasmtime::Config, String> {
    let mut config = wasmtime::Config::new();
    config.wasm_multi_memory(true);
    config.cranelift_nan_canonicalization(true);
    config.parallel_compilation(true);
    config.target(&target.triple()).map_err(|e| format!("target {}: {e}", target.triple()))?;
    for flag in target.flags() {
        // SAFETY: every flag is one of Cranelift's own CPU-feature booleans for this architecture
        // (X86_64_V2, APPLE_M1). Code that uses a feature the CPU lacks is refused by wasmtime when
        // it is compiled or loaded for this machine, never run.
        unsafe {
            config.cranelift_flag_enable(flag);
        }
    }
    Ok(config)
}

/// The Backend runner's settings (spec 114 D3): [`config`] plus **epoch interruption**, so a call that must end (the
/// caller gave up, its time is up) is stopped from outside by advancing the engine's epoch, whatever the program is
/// doing. The checks are in the machine code, so code made for this configuration and code made for [`config`] are not
/// each other's: their compatibility hashes differ and [`load`] passes by the one it cannot use. [`config`] is not
/// edited, so no Desktop module's machine code moves. **A store of this engine starts with its epoch deadline at
/// zero, so a call traps at its first check until the caller sets one** (`Store::set_epoch_deadline`).
pub fn backend_config(target: &Target) -> Result<wasmtime::Config, String> {
    let mut config = config(target)?;
    config.epoch_interruption(true);
    Ok(config)
}

/// An engine for `target` with [`backend_config`].
pub fn backend_engine_for(target: &Target) -> Result<Engine, String> {
    Engine::new(&backend_config(target)?).map_err(|e| e.to_string())
}

/// The engine a Backend's calls run on here: [`Target::host`].
pub fn backend_engine() -> Result<Engine, String> {
    backend_engine_for(&Target::host())
}

/// The Build's machine code for a Backend's `module` on `target`: made by [`backend_engine_for`], so only a Backend
/// engine loads it.
pub fn precompile_backend(module: &[u8], target: &Target) -> Result<MachineCode, String> {
    let engine = backend_engine_for(target)?;
    let bytes = engine.precompile_module(module).map_err(|e| e.to_string())?;
    Ok(MachineCode { sha256: sha256(&bytes), compat: compatibility_hash(&engine), bytes })
}

/// How large the instance pool of a Backend runner is: instances held ready, and what one of them may have.
#[cfg(feature = "pooling")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pool {
    /// The most instances alive at once (the calls in flight).
    pub instances: u32,
    /// The memories one module declares (the generated modules have two: the workspace and the arena).
    pub memories_per_module: u32,
    /// The most bytes one memory may reach.
    pub max_memory_bytes: usize,
}

/// [`backend_config`] with a pooling allocator sized by `pool`: an instance is taken from a pool made once, not mapped
/// and unmapped for each call. The code it runs is the same, so machine code made by [`precompile_backend`] loads in
/// the pooled engine too (the compatibility hashes are equal; a test holds it).
#[cfg(feature = "pooling")]
pub fn backend_pooled_config(target: &Target, pool: &Pool) -> Result<wasmtime::Config, String> {
    let mut config = backend_config(target)?;
    let mut sizing = wasmtime::PoolingAllocationConfig::default();
    sizing
        .total_core_instances(pool.instances)
        .total_memories(pool.instances.saturating_mul(pool.memories_per_module))
        .max_memories_per_module(pool.memories_per_module)
        .total_tables(pool.instances)
        .max_memory_size(pool.max_memory_bytes);
    config.allocation_strategy(wasmtime::InstanceAllocationStrategy::Pooling(sizing));
    Ok(config)
}

/// An engine for `target`. A target other than this machine's can make machine code
/// ([`precompile`]) but cannot load it.
pub fn engine_for(target: &Target) -> Result<Engine, String> {
    Engine::new(&config(target)?).map_err(|e| e.to_string())
}

/// The engine generated modules run on here: [`Target::host`].
pub fn engine() -> Result<Engine, String> {
    engine_for(&Target::host())
}

/// The sha256 of `bytes`.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// wasmtime's precompile compatibility hash of `engine`, as a sha256: equal for two engines means
/// machine code from one deserialises in the other (`Engine::precompile_compatibility_hash`).
pub fn compatibility_hash(engine: &Engine) -> [u8; 32] {
    struct Sha(Sha256);
    impl Hasher for Sha {
        fn write(&mut self, bytes: &[u8]) {
            self.0.update(bytes);
        }
        fn finish(&self) -> u64 {
            let d: [u8; 32] = self.0.clone().finalize().into();
            u64::from_le_bytes([d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]])
        }
    }
    let mut h = Sha(Sha256::new());
    engine.precompile_compatibility_hash().hash(&mut h);
    h.0.finalize().into()
}

/// Machine code made ahead for one module, with what [`load`] checks before trusting it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineCode {
    /// wasmtime's serialised module.
    pub bytes: Vec<u8>,
    /// The sha256 of `bytes` when they were made.
    pub sha256: [u8; 32],
    /// The [`compatibility_hash`] of the engine that made them.
    pub compat: [u8; 32],
}

/// The Build's machine code for `module` on `target`.
pub fn precompile(module: &[u8], target: &Target) -> Result<MachineCode, String> {
    let engine = engine_for(target)?;
    let bytes = engine.precompile_module(module).map_err(|e| e.to_string())?;
    Ok(MachineCode { sha256: sha256(&bytes), compat: compatibility_hash(&engine), bytes })
}

/// The path [`load`] took to a runnable module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadPath {
    /// The machine code the Build made.
    MachineCode,
    /// The machine code an earlier run compiled and kept in the local cache.
    Cache,
    /// Compiled now, at load (and kept in the cache, when there is one).
    Compiled,
}

/// Why [`load`] passed the Build's machine code by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Skipped {
    /// The program carries none.
    Absent,
    /// Its bytes do not match their sha256: damaged or cut short.
    Damaged,
    /// It was made by an engine this one cannot load from: another wasmtime, other settings or
    /// another CPU level.
    Foreign,
    /// wasmtime refused it, with its own words.
    Refused(String),
}

/// A module ready to instantiate, and how it was reached.
pub struct Loaded {
    pub module: Module,
    pub path: LoadPath,
    /// `None` when the Build's machine code was used.
    pub skipped: Option<Skipped>,
    /// Milliseconds from the call to a runnable module, a cache write included.
    pub ms: f64,
}

/// Load `module` for `engine`: the Build's `code` when its sha256 and compatibility hash hold and
/// wasmtime accepts it; otherwise the `cache`'s copy; otherwise compile at load and keep the result
/// in the `cache`. Fails only when the module itself does not compile.
pub fn load(engine: &Engine, module: &[u8], code: Option<&MachineCode>, cache: Option<&Cache>) -> Result<Loaded, String> {
    let started = Instant::now();
    let ms = |s: Instant| s.elapsed().as_secs_f64() * 1000.0;
    let compat = compatibility_hash(engine);
    let skipped = match code {
        None => Skipped::Absent,
        Some(c) if sha256(&c.bytes) != c.sha256 => Skipped::Damaged,
        Some(c) if c.compat != compat => Skipped::Foreign,
        // SAFETY: the bytes are the Build's own `precompile` output: their sha256 is the one recorded
        // when they were made, and the engine that made them hashes equal to this one.
        Some(c) => match unsafe { Module::deserialize(engine, &c.bytes) } {
            Ok(m) => return Ok(Loaded { module: m, path: LoadPath::MachineCode, skipped: None, ms: ms(started) }),
            Err(e) => Skipped::Refused(e.to_string()),
        },
    };
    let key = cache_key(module, &compat);
    if let Some(m) = cache.and_then(|c| c.get(engine, &key)) {
        return Ok(Loaded { module: m, path: LoadPath::Cache, skipped: Some(skipped), ms: ms(started) });
    }
    let m = Module::new(engine, module).map_err(|e| e.to_string())?;
    if let Some(c) = cache {
        if let Ok(bytes) = m.serialize() {
            c.put(&key, &bytes);
        }
    }
    Ok(Loaded { module: m, path: LoadPath::Compiled, skipped: Some(skipped), ms: ms(started) })
}

/// The cache entry's name: the module and the engine it was compiled by.
fn cache_key(module: &[u8], compat: &[u8; 32]) -> String {
    let mut h = Sha256::new();
    h.update(sha256(module));
    h.update(compat);
    let d: [u8; 32] = h.finalize().into();
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// Machine code compiled at load, kept on disk for the next run: one `<key>.cwasm` per module and
/// engine, each its sha256 followed by wasmtime's serialised module. Written whole and renamed into
/// place; read only when its sha256 holds; the least recently used go first past the limit. A cache
/// that cannot be read or written is passed by: it never stops a program.
#[derive(Clone, Debug)]
pub struct Cache {
    dir: PathBuf,
    limit: u64,
}

impl Cache {
    /// A cache in `dir` (made when first written) holding at most `limit` bytes.
    pub fn new(dir: impl Into<PathBuf>, limit: u64) -> Self {
        Self { dir: dir.into(), limit }
    }

    /// The folder the cache lives in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn file(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.cwasm"))
    }

    fn get(&self, engine: &Engine, key: &str) -> Option<Module> {
        let path = self.file(key);
        let bytes = fs::read(&path).ok()?;
        let good = bytes.len() > 32 && sha256(&bytes[32..])[..] == bytes[..32];
        // SAFETY: the bytes are this cache's own entry, written by `put` from `Module::serialize` of
        // an engine whose compatibility hash is part of the key, and their sha256 holds.
        let module = good.then(|| unsafe { Module::deserialize(engine, &bytes[32..]) }.ok()).flatten();
        match &module {
            Some(_) => {
                let _ = fs::File::options().write(true).open(&path).and_then(|f| f.set_modified(SystemTime::now()));
            }
            None => {
                let _ = fs::remove_file(&path);
            }
        }
        module
    }

    fn put(&self, key: &str, code: &[u8]) {
        if fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        let tmp = self.dir.join(format!("{key}.{}.tmp", std::process::id()));
        let written = fs::File::create(&tmp).and_then(|mut f| {
            f.write_all(&sha256(code))?;
            f.write_all(code)?;
            f.sync_all()
        });
        if written.is_err() || fs::rename(&tmp, self.file(key)).is_err() {
            let _ = fs::remove_file(&tmp);
            return;
        }
        self.trim();
    }

    /// Remove the least recently used entries until the cache fits its limit.
    fn trim(&self) {
        let Ok(dir) = fs::read_dir(&self.dir) else { return };
        let mut entries: Vec<(SystemTime, u64, PathBuf)> = dir
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "cwasm"))
            .filter_map(|e| {
                let m = e.metadata().ok()?;
                Some((m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len(), e.path()))
            })
            .collect();
        let mut total: u64 = entries.iter().map(|e| e.1).sum();
        entries.sort();
        for (_, len, path) in entries {
            if total <= self.limit {
                break;
            }
            if fs::remove_file(&path).is_ok() {
                total -= len;
            }
        }
    }
}
