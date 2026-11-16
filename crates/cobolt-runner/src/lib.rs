// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **`prc-runner`, the PowerRustCOBOL runner** (DESKTOP-WASM-001 T15, plan §1.1 and §4).
//!
//! A Desktop program built on the WebAssembly path is a copy of this executable with the program
//! inside it (T14). When it starts, the runner:
//!
//! 1. reads the payload from its own file (`PRLOC` overlay, or the `__PRPAYLOAD` segment);
//! 2. reads the module's manifest and accepts every ABI from 1 to [`ABI_VERSION`](cobolt_wasm_abi::codes::ABI_VERSION) (R5, C2);
//! 3. refuses, naming them, the capabilities and imports it does not provide (R4), in the system's
//!    language ([`messages`]);
//! 4. loads the Build's machine code, or the cache's, or compiles at load (T13, R16);
//! 5. links exactly the imports the module declares ([`linker_for`]): `pr` for the run, and the
//!    console part of `prn` (A27, A29, A30);
//! 6. runs the program to its end and exits with the code it handed to `prn.exit` (0 when none).
//!
//! What `DISPLAY` writes goes to standard output, a line per command; `ACCEPT` from the console
//! reads standard input; the environment and the arguments are this process's own. The clock and
//! the random source honour the test pins ([`pins`], R28).
//!
//! The runner never names itself after its crate: its words are "the PowerRustCOBOL runner".

pub mod messages;
pub mod pins;

use std::io::{BufRead, Write};
use std::path::PathBuf;

use cobolt_program_package::{Kind, PackageError};
use cobolt_runner_engine::{Cache, MachineCode};
use cobolt_wasm_abi::batch::{decode_batch, Command};
use cobolt_wasm_abi::codes::{cap, IMPORT_MODULE, NATIVE_IMPORT_MODULE};
use cobolt_wasm_abi::manifest::{Manifest, ManifestError, MANIFEST_SECTION};
use wasmtime::{Caller, Engine, Linker, Memory, Store};

pub use messages::{Lang, Msg};
pub use pins::Pins;

/// The capabilities this runner provides (A27). T15: console programs.
pub const PROVIDED_CAPABILITIES: &[&str] = &[cap::CONSOLE, cap::ENV];

/// Every import this runner provides, with the ABI that introduced it: `(abi, module, name)`. A row
/// is never removed, so a module of any earlier ABI finds every import it was built with (R5).
/// Not yet provided: `pr.query` and `pr.remote` (forms and the backend, M2), and the conformance
/// imports, which a desktop Build never makes.
pub const PROVIDED_IMPORTS: &[(u32, &str, &str)] = &[
    (1, IMPORT_MODULE, "flush"),
    (1, IMPORT_MODULE, "abort"),
    (1, IMPORT_MODULE, "now"),
    (1, IMPORT_MODULE, "random"),
    (1, NATIVE_IMPORT_MODULE, "con_read"),
    (1, NATIVE_IMPORT_MODULE, "env_get"),
    (1, NATIVE_IMPORT_MODULE, "exit"),
    (1, NATIVE_IMPORT_MODULE, "tz_offset"),
];

/// `pr_run`'s answers (the generator's `ret`): the run finished, or `STOP RUN`.
const RUN_DONE: i32 = -1;
const RUN_STOP: i32 = -4;
/// `con_read` and `env_get`: the end of the input, and "not there".
const END_OF_INPUT: i32 = -10;
const NOT_THERE: i32 = -1;

/// Run the program inside this executable and answer its exit code.
pub fn run_self() -> i32 {
    let lang = Lang::system();
    let exe = match std::env::current_exe().and_then(std::fs::read) {
        Ok(b) => b,
        Err(e) => return refuse(lang, Msg::CannotStart(e.to_string())),
    };
    run_executable(&exe, lang)
}

/// Run the program `exe` (an executable's bytes) carries.
pub fn run_executable(exe: &[u8], lang: Lang) -> i32 {
    let payload = match cobolt_program_package::locate(exe) {
        Ok(p) => p,
        Err(PackageError::NoPayload) => return refuse(lang, Msg::NoProgram),
        Err(e) => return refuse(lang, Msg::Damaged(e.to_string())),
    };
    let payload = match cobolt_program_package::decode(payload) {
        Ok(p) => p,
        Err(e) => return refuse(lang, Msg::Damaged(e.to_string())),
    };
    let module = match payload.module() {
        Ok(m) => m,
        Err(e) => return refuse(lang, Msg::Damaged(e.to_string())),
    };
    let code = payload.part(Kind::MachineCode).zip(payload.part(Kind::MachineCodeEngine)).and_then(|(code, engine)| {
        let compat: [u8; 32] = engine.bytes.try_into().ok()?;
        Some(MachineCode { bytes: code.bytes.to_vec(), sha256: code.sha256, compat })
    });
    run_module(module, code.as_ref(), lang)
}

/// Check, load and run `module`, with the Build's machine code when there is some.
pub fn run_module(module: &[u8], code: Option<&MachineCode>, lang: Lang) -> i32 {
    let checked = match check(module) {
        Ok(c) => c,
        Err(msg) => return refuse(lang, msg),
    };
    let (pins, said) = Pins::from_env();
    for s in said {
        eprintln!("{s}");
    }
    let engine = match cobolt_runner_engine::engine() {
        Ok(e) => e,
        Err(e) => return refuse(lang, Msg::CannotStart(e)),
    };
    let cache = cache_dir().map(|d| Cache::new(d, cobolt_runner_engine::CACHE_LIMIT));
    let loaded = match cobolt_runner_engine::load(&engine, module, code, cache.as_ref()) {
        Ok(l) => l,
        Err(e) => return refuse(lang, Msg::CannotStart(e)),
    };
    if std::env::var_os("PRC_RUNNER_TRACE").is_some() {
        let why = loaded.skipped.as_ref().map(|s| format!(" ({s:?})")).unwrap_or_default();
        eprintln!("prc-runner: loaded by {:?}{why} in {:.1} ms", loaded.path, loaded.ms);
    }
    let linker = match linker_for(&engine, checked.abi, &checked.imports) {
        Ok(l) => l,
        Err(msg) => return refuse(lang, msg),
    };
    let mut store = Store::new(&engine, State::new(lang, pins));
    let instance = match linker.instantiate(&mut store, &loaded.module) {
        Ok(i) => i,
        Err(e) => return refuse(lang, Msg::CannotStart(format!("{e:#}"))),
    };
    let started = (|| -> Result<wasmtime::TypedFunc<(), i32>, String> {
        let arena = instance.get_memory(&mut store, "arena").ok_or("the module exports no `arena` memory")?;
        let base = instance.get_typed_func::<(), i32>(&mut store, "pr_arena_base").map_err(|e| e.to_string())?;
        let base = base.call(&mut store, ()).map_err(|e| e.to_string())?;
        store.data_mut().arena = Some(arena);
        store.data_mut().arena_base = base as u32;
        let abi = instance.get_typed_func::<(), i32>(&mut store, "pr_abi").map_err(|e| e.to_string())?;
        let abi = abi.call(&mut store, ()).map_err(|e| e.to_string())?;
        if abi as u32 != checked.abi {
            return Err(format!("the module answers ABI {abi} and its manifest names ABI {}", checked.abi));
        }
        instance.get_typed_func::<(), i32>(&mut store, "pr_run").map_err(|e| e.to_string())
    })();
    let run = match started {
        Ok(r) => r,
        Err(e) => return refuse(lang, Msg::CannotStart(e)),
    };
    let result = run.call(&mut store, ());
    let _ = std::io::stdout().flush();
    let st = store.data_mut();
    if let Some(msg) = st.fatal.take() {
        return refuse(lang, msg);
    }
    match result {
        Ok(RUN_DONE | RUN_STOP) => st.exit_code.unwrap_or(0),
        Ok(other) => refuse(lang, Msg::Trapped(format!("the program stopped waiting for an event a console program never gets (status {other})"))),
        Err(e) => refuse(lang, Msg::Trapped(format!("{e:#}"))),
    }
}

/// What a module needs, read before it is loaded.
struct Checked {
    abi: u32,
    imports: Vec<(String, String)>,
}

/// Read the module's manifest and imports: the ABI must be one this runner runs (1 to
/// [`ABI_VERSION`](cobolt_wasm_abi::codes::ABI_VERSION)), and every capability and import one it provides.
fn check(module: &[u8]) -> Result<Checked, Msg> {
    let scan = cobolt_wasm_abi::scan::scan_module(module).map_err(Msg::Damaged)?;
    let text = scan.custom(MANIFEST_SECTION).ok_or_else(|| Msg::Damaged(format!("the module has no `{MANIFEST_SECTION}` section")))?;
    let manifest: Manifest = std::str::from_utf8(text)
        .ok()
        .and_then(|t| serde_json::from_str(t).ok())
        .ok_or_else(|| Msg::Damaged("the module's manifest cannot be read".into()))?;
    match manifest.validate() {
        Ok(()) => {}
        Err(ManifestError::AbiMismatch { found, expected }) => return Err(Msg::Abi { found, max: expected }),
        Err(e) => return Err(Msg::Damaged(e.to_string())),
    }
    let missing: Vec<String> =
        manifest.capabilities.iter().filter(|c| !PROVIDED_CAPABILITIES.contains(&c.as_str())).cloned().collect();
    if !missing.is_empty() {
        return Err(Msg::Capabilities(missing));
    }
    let imports: Vec<(String, String)> = scan.imports.iter().map(|i| (i.module.clone(), i.name.clone())).collect();
    if let Some(name) = missing_imports(manifest.abi, &imports).into_iter().next() {
        return Err(Msg::Import(name));
    }
    Ok(Checked { abi: manifest.abi, imports })
}

/// The imports in `imports` this runner does not provide to a module of `abi`, as `module.name`.
pub fn missing_imports(abi: u32, imports: &[(String, String)]) -> Vec<String> {
    imports
        .iter()
        .filter(|(m, n)| !PROVIDED_IMPORTS.iter().any(|(since, pm, pn)| *since <= abi && pm == m && pn == n))
        .map(|(m, n)| format!("{m}.{n}"))
        .collect()
}

/// Say `msg` on standard error, in `lang`, and answer the exit code a refused run ends with.
fn refuse(lang: Lang, msg: Msg) -> i32 {
    let _ = std::io::stdout().flush();
    eprintln!("{}", messages::text(lang, &msg));
    1
}

/// Where machine code compiled at load is kept: `PRC_RUNNER_CACHE` when it is set, else the
/// user's cache folder; `None` when there is none.
fn cache_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("PRC_RUNNER_CACHE") {
        return Some(PathBuf::from(d));
    }
    let home = || std::env::var_os("HOME").map(PathBuf::from);
    let base = if cfg!(target_os = "macos") {
        home().map(|h| h.join("Library").join("Caches"))
    } else if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).or_else(|| home().map(|h| h.join(".cache")))
    };
    base.map(|b| b.join("PowerRustCOBOL").join("runner"))
}

// ── the imports ─────────────────────────────────────────────────────────────

/// What the imports share during a run.
pub struct State {
    lang: Lang,
    pins: Pins,
    random: u64,
    arena: Option<Memory>,
    arena_base: u32,
    exit_code: Option<i32>,
    /// What ended the run, when the module aborted.
    fatal: Option<Msg>,
}

impl State {
    fn new(lang: Lang, pins: Pins) -> Self {
        State { lang, random: pins.random_state(), pins, arena: None, arena_base: 0, exit_code: None, fatal: None }
    }
}

/// The batch the module staged at the start of its arena.
fn read_batch(caller: &Caller<'_, State>, len: i32) -> Vec<u8> {
    let st = caller.data();
    let mut buf = vec![0u8; len.max(0) as usize];
    if let Some(mem) = st.arena {
        let _ = mem.read(caller, st.arena_base as usize, &mut buf);
    }
    buf
}

/// Carry out a batch: `DISPLAY` lines to standard output, errors to standard error. In an abort
/// the first error ends the run and is said once, after the output.
fn apply(batch: &[u8], st: &mut State, abort: bool) {
    let out = std::io::stdout();
    let mut out = out.lock();
    match decode_batch(batch) {
        Ok(cmds) => {
            for c in cmds {
                match c {
                    Command::Display(t) => {
                        let _ = out.write_all(t.as_bytes());
                        let _ = out.write_all(b"\n");
                    }
                    Command::Error { code, line, message, .. } => {
                        let msg = Msg::RuntimeError { code, line, text: message.to_string() };
                        if abort {
                            st.fatal.get_or_insert(msg);
                        } else {
                            let _ = out.flush();
                            eprintln!("{}", messages::text(st.lang, &msg));
                        }
                    }
                    // A console program has no controls: nothing else reaches it.
                    _ => {}
                }
            }
        }
        Err(e) => {
            st.fatal.get_or_insert(Msg::Trapped(format!("the program sent a batch that cannot be read: {e}")));
        }
    }
    let _ = out.flush();
}

/// The data memory, where `prn` imports read and write (A29).
fn memory0(caller: &mut Caller<'_, State>) -> Option<Memory> {
    caller.get_export("memory").and_then(|e| e.into_memory())
}

/// A linker with exactly `imports`, each as this runner implements it for a module of `abi`; an
/// import it does not provide is named.
pub fn linker_for(engine: &Engine, abi: u32, imports: &[(String, String)]) -> Result<Linker<State>, Msg> {
    if let Some(name) = missing_imports(abi, imports).into_iter().next() {
        return Err(Msg::Import(name));
    }
    let mut linker: Linker<State> = Linker::new(engine);
    let fail = |e: wasmtime::Error| Msg::CannotStart(e.to_string());
    for (module, name) in imports {
        match (module.as_str(), name.as_str()) {
            (IMPORT_MODULE, "flush") => {
                linker
                    .func_wrap(IMPORT_MODULE, "flush", |mut caller: Caller<'_, State>, len: i32| {
                        let batch = read_batch(&caller, len);
                        apply(&batch, caller.data_mut(), false);
                    })
                    .map_err(fail)?;
            }
            (IMPORT_MODULE, "abort") => {
                linker
                    .func_wrap(IMPORT_MODULE, "abort", |mut caller: Caller<'_, State>, len: i32| -> wasmtime::Result<()> {
                        let batch = read_batch(&caller, len);
                        apply(&batch, caller.data_mut(), true);
                        Err(wasmtime::Error::msg("run-time error"))
                    })
                    .map_err(fail)?;
            }
            (IMPORT_MODULE, "now") => {
                linker.func_wrap(IMPORT_MODULE, "now", |caller: Caller<'_, State>| -> f64 { caller.data().pins.now_ms() }).map_err(fail)?;
            }
            (IMPORT_MODULE, "random") => {
                linker
                    .func_wrap(IMPORT_MODULE, "random", |mut caller: Caller<'_, State>| -> f64 {
                        let st = caller.data_mut();
                        let (next, v) = pins::lcg_step(st.random);
                        st.random = next;
                        v
                    })
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "con_read") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "con_read", |mut caller: Caller<'_, State>, addr: i32, cap: i32| -> i32 {
                        let _ = std::io::stdout().flush();
                        let mut line = Vec::new();
                        match std::io::stdin().lock().read_until(b'\n', &mut line) {
                            Ok(0) | Err(_) => return END_OF_INPUT,
                            Ok(_) => {}
                        }
                        while matches!(line.last(), Some(b'\n' | b'\r')) {
                            line.pop();
                        }
                        line.truncate(cap.max(0) as usize);
                        let Some(mem) = memory0(&mut caller) else { return END_OF_INPUT };
                        mem.write(&mut caller, addr as u32 as usize, &line).map(|_| line.len() as i32).unwrap_or(END_OF_INPUT)
                    })
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "env_get") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "env_get", |mut caller: Caller<'_, State>, kind: i32, n: i32, buf: i32, cap: i32| -> i32 {
                        let Some(mem) = memory0(&mut caller) else { return NOT_THERE };
                        let args = || std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned());
                        let text = match kind {
                            0 => {
                                let mut name = vec![0u8; n.max(0) as usize];
                                if mem.read(&caller, buf as u32 as usize, &mut name).is_err() {
                                    return NOT_THERE;
                                }
                                env_var(&name)
                            }
                            1 => (n as u32 as usize).checked_sub(1).and_then(|i| args().nth(i)),
                            2 => return args().count() as i32,
                            3 => Some(args().collect::<Vec<_>>().join(" ")),
                            _ => None,
                        };
                        let Some(text) = text else { return NOT_THERE };
                        let bytes = &text.as_bytes()[..text.len().min(cap.max(0) as usize)];
                        mem.write(&mut caller, buf as u32 as usize, bytes).map(|_| bytes.len() as i32).unwrap_or(NOT_THERE)
                    })
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "tz_offset") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "tz_offset", |caller: Caller<'_, State>| -> i32 { caller.data().pins.offset_minutes() })
                    .map_err(fail)?;
            }
            (NATIVE_IMPORT_MODULE, "exit") => {
                linker
                    .func_wrap(NATIVE_IMPORT_MODULE, "exit", |mut caller: Caller<'_, State>, code: i32| {
                        caller.data_mut().exit_code = Some(code);
                    })
                    .map_err(fail)?;
            }
            // `missing_imports` let only the rows of PROVIDED_IMPORTS through.
            _ => return Err(Msg::Import(format!("{module}.{name}"))),
        }
    }
    Ok(linker)
}

/// The environment variable named by these bytes, as text; `None` when it is not set.
fn env_var(name: &[u8]) -> Option<String> {
    #[cfg(unix)]
    let name = {
        use std::os::unix::ffi::OsStrExt;
        std::ffi::OsStr::from_bytes(name).to_os_string()
    };
    #[cfg(not(unix))]
    let name = std::ffi::OsString::from(String::from_utf8_lossy(name).into_owned());
    // A name no system can hold is not set (and is never handed to the system's lookup).
    if name.is_empty() || name.as_encoded_bytes().iter().any(|b| matches!(b, b'=' | 0)) {
        return None;
    }
    std::env::var_os(name).map(|v| v.to_string_lossy().into_owned())
}
