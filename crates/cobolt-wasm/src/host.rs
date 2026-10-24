// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A host for generated modules, over `wasmtime`: it implements ABI v1 for the
//! tests and for the NIST gate. It is *not* the browser host (that is the
//! player's) — it is the smallest thing that can stand in for one, which is
//! what makes the conformance numbers about the module and not about a page.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::rc::Rc;
use cobolt_wasm_abi::batch::{decode_batch, Command};
use wasmtime::{Caller, Engine, Instance, Linker, Memory, Module, Store};

/// Where a conformance run finds and creates its files.
#[derive(Debug, Clone)]
pub struct Conformance {
    pub cwd: PathBuf,
    /// External switches, by implementor name or mnemonic (case-insensitive).
    pub switches: Vec<(String, bool)>,
}

enum OpenFile {
    Reader(std::io::BufReader<std::fs::File>),
    Writer(std::fs::File),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtError {
    pub code: u32,
    pub line: u32,
}

#[derive(Debug, Default, Clone)]
pub struct Outcome {
    /// `pr_run`'s status: -1 done, -4 stop run, or a trap.
    pub status: i32,
    /// What `DISPLAY` wrote.
    pub output: String,
    pub errors: Vec<RtError>,
    /// Number of `flush` calls the module made.
    pub flushes: u32,
    pub trapped: Option<String>,
    /// The commands other than `DISPLAY` and `ERROR`, as the host received them
    /// (`SET`/`CALL`/`LOG`, rendered with their numeric path).
    pub commands: Vec<String>,
    /// The `SET` commands: `(instance, control, name ids of the path, value)`.
    pub sets: Vec<(u32, u32, Vec<u32>, String)>,
    /// The `CALL` commands: `(instance, control, name ids, arguments of the last segment)`.
    pub calls: Vec<(u32, u32, Vec<u32>, Vec<String>)>,
    /// Number of `query` calls the module made.
    pub queries: u32,
    /// The full `ERROR` commands: `(code, unit, line, form, event)`.
    pub error_positions: Vec<(u32, u32, u32, u32, u32)>,
    /// The `SET_NAMED` commands (A14): `(instance, control, property, value)`.
    pub named_sets: Vec<(u32, u32, String, String)>,
    /// The `RAISE` commands (A14): `(instance, control, event, argument)`.
    pub raised: Vec<(u32, u32, String, String)>,
}

struct State {
    arena: Option<Memory>,
    arena_base: u32,
    answer_at: u32,
    out: Rc<RefCell<Outcome>>,
    conformance: Option<Conformance>,
    files: HashMap<u32, OpenFile>,
    next_handle: u32,
    /// The properties the module has set: `(instance, control, name id)` -> text.
    props: HashMap<(u32, u32, u32), String>,
    /// What `now` and `random` answer (R13: fixed answers, deterministic runs).
    now_ms: Option<f64>,
    random: f64,
    /// The request frames the module sent with `remote`, in order.
    remote_frames: Vec<Vec<u8>>,
    /// Every batch the module flushed, byte for byte, when asked to keep them.
    batches: Option<Vec<Vec<u8>>>,
}

/// Apply a command batch to the outcome, through the ABI's one codec.
fn apply(batch: &[u8], out: &mut Outcome) {
    let cmds = match decode_batch(batch) {
        Ok(c) => c,
        Err(e) => {
            out.errors.push(RtError { code: 9000, line: 0 });
            out.trapped.get_or_insert_with(|| format!("bad batch: {e}"));
            return;
        }
    };
    for c in cmds {
        match c {
            // One command is one line of the console (ABI §5, WASM-010 T21).
            Command::Display(t) => {
                out.output.push_str(t);
                out.output.push('\n');
            }
            Command::Error { code, line, unit, form, event, .. } => {
                out.errors.push(RtError { code, line });
                out.error_positions.push((code, unit, line, form, event));
            }
            Command::Set { path, value } => {
                out.sets.push((path.inst, path.ctrl, path.segs.iter().map(|s| s.name).collect(), value.to_string()));
                out.commands.push(format!("{:?}", Command::Set { path, value }));
            }
            Command::Call { path } => {
                let args = path.last().map(|s| s.args.iter().map(|a| a.to_string()).collect()).unwrap_or_default();
                out.calls.push((path.inst, path.ctrl, path.segs.iter().map(|s| s.name).collect(), args));
                out.commands.push(format!("{:?}", Command::Call { path }));
            }
            Command::SetNamed { inst, ctrl, name, value } => {
                out.named_sets.push((inst, ctrl, name.to_string(), value.to_string()));
                out.commands.push(format!("{c:?}"));
            }
            Command::Raise { inst, ctrl, event, arg } => {
                out.raised.push((inst, ctrl, event.to_string(), arg.to_string()));
                out.commands.push(format!("{c:?}"));
            }
            other => out.commands.push(format!("{other:?}")),
        }
    }
}

/// Keep the value of every plain-property `SET` of a batch, so that a later
/// `GET` reads the handler's own earlier write (§6, WASM-017 R11).
fn remember_sets(batch: &[u8], props: &mut HashMap<(u32, u32, u32), String>) {
    for c in decode_batch(batch).unwrap_or_default() {
        if let Command::Set { path, value } = c {
            let segs: Vec<u32> = path.segs.iter().map(|s| s.name).collect();
            if let Some(k) = prop_key(path.inst, path.ctrl, &segs) {
                props.insert(k, value.to_string());
            }
        }
    }
}

/// The property a `SET` of this path names, for the host's property store.
fn prop_key(inst: u32, ctrl: u32, segs: &[u32]) -> Option<(u32, u32, u32)> {
    match segs {
        [name] => Some((inst, ctrl, *name)),
        _ => None,
    }
}

fn read_batch(caller: &Caller<'_, State>, len: u32) -> Vec<u8> {
    let st = caller.data();
    let mem = st.arena.expect("arena");
    let mut buf = vec![0u8; len as usize];
    mem.read(caller, st.arena_base as usize, &mut buf).expect("read arena");
    buf
}

/// The engine every generated module runs on. Multi-memory for the arena
/// (WASM-017 R9); NaN canonicalisation so `COMP-1`/`COMP-2` give the same bits
/// on every machine (R13).
pub fn engine() -> Result<Engine, String> {
    let mut config = wasmtime::Config::new();
    config.wasm_multi_memory(true);
    config.cranelift_nan_canonicalization(true);
    Engine::new(&config).map_err(|e| e.to_string())
}

/// Run a program-mode module (`pr_run`) to its end.
pub fn run_program(wasm: &[u8]) -> Result<Outcome, String> {
    run_with(wasm, None, 64 * 1024, false).map(|(o, _)| o)
}

/// [`run_program`], and the module's data memory as the run left it (the
/// byte-layout tests read their items out of it).
pub fn run_program_memory(wasm: &[u8]) -> Result<(Outcome, Vec<u8>), String> {
    run_with(wasm, None, 64 * 1024, true)
}

/// Run a conformance module: file verbs work against `conf.cwd`.
pub fn run_conformance(wasm: &[u8], conf: Conformance) -> Result<Outcome, String> {
    run_with(wasm, Some(conf), 64 * 1024, false).map(|(o, _)| o)
}

fn new_state(out: Rc<RefCell<Outcome>>, conformance: Option<Conformance>, batch_size: u32) -> State {
    State {
        arena: None,
        arena_base: 0,
        answer_at: batch_size,
        out,
        conformance,
        files: HashMap::new(),
        next_handle: 1,
        props: HashMap::new(),
        now_ms: None,
        random: 0.5,
        remote_frames: Vec::new(),
        batches: None,
    }
}

/// Every import of ABI v1 (§3 and the conformance imports of §8), except the
/// ones named in `omit` — a host that lacks one of them, for AC9.
fn linker(engine: &Engine, omit: &[&str]) -> Result<Linker<State>, String> {
    let mut linker: Linker<State> = Linker::new(engine);
    let want = |n: &str| !omit.contains(&n);

    linker
        .func_wrap("pr", "flush", |mut caller: Caller<'_, State>, len: i32| {
            let batch = read_batch(&caller, len as u32);
            let out = caller.data().out.clone();
            {
                let mut o = out.borrow_mut();
                o.flushes += 1;
                apply(&batch, &mut o);
            }
            if let Some(b) = caller.data_mut().batches.as_mut() {
                b.push(batch.clone());
            }
            remember_sets(&batch, &mut caller.data_mut().props);
        })
        .map_err(|e| e.to_string())?;
    linker
        .func_wrap("pr", "abort", |caller: Caller<'_, State>, len: i32| -> wasmtime::Result<()> {
            let batch = read_batch(&caller, len as u32);
            let out = caller.data().out.clone();
            apply(&batch, &mut out.borrow_mut());
            Err(wasmtime::Error::msg("run-time error"))
        })
        .map_err(|e| e.to_string())?;
    // External switches (ABI v1 §8): 1 on, 0 off, -1 unknown.
    linker
        .func_wrap("pr", "switch_state", |caller: Caller<'_, State>, len: i32| -> i32 {
            let name = String::from_utf8_lossy(&read_batch(&caller, len as u32)).into_owned();
            let Some(conf) = caller.data().conformance.as_ref() else { return -1 };
            match conf.switches.iter().find(|(k, _)| k.eq_ignore_ascii_case(name.trim())) {
                Some((_, on)) => i32::from(*on),
                None => -1,
            }
        })
        .map_err(|e| e.to_string())?;
    // Conformance files (ABI v1 §8).
    linker
        .func_wrap("pr", "file_open", |mut caller: Caller<'_, State>, len: i32, mode: i32| -> i32 {
            let name = String::from_utf8_lossy(&read_batch(&caller, len as u32)).into_owned();
            let Some(conf) = caller.data().conformance.clone() else { return -37 };
            let path = conf.cwd.join(name.trim());
            let m = (mode & 0xFF) as u32;
            let opened = match m {
                0 => std::fs::File::open(&path).map(|f| OpenFile::Reader(std::io::BufReader::new(f))),
                1 => std::fs::File::create(&path).map(OpenFile::Writer),
                3 => std::fs::OpenOptions::new().append(true).create(true).open(&path).map(OpenFile::Writer),
                _ => std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&path).map(|f| OpenFile::Reader(std::io::BufReader::new(f))),
            };
            match opened {
                Ok(f) => {
                    let st = caller.data_mut();
                    let h = st.next_handle;
                    st.next_handle += 1;
                    st.files.insert(h, f);
                    h as i32
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => -35,
                Err(_) => -37,
            }
        })
        .map_err(|e| e.to_string())?;
    linker
        .func_wrap("pr", "file_close", |mut caller: Caller<'_, State>, handle: i32| -> i32 {
            match caller.data_mut().files.remove(&(handle as u32)) {
                Some(OpenFile::Writer(mut f)) => {
                    let _ = f.flush();
                    0
                }
                Some(_) => 0,
                None => -42,
            }
        })
        .map_err(|e| e.to_string())?;
    linker
        .func_wrap("pr", "file_write", |mut caller: Caller<'_, State>, handle: i32, len: i32, adv: i32| -> i32 {
            let rec = read_batch(&caller, len as u32);
            let adv = adv as u32;
            let mut text: Vec<u8> = Vec::new();
            let none = adv == u32::MAX;
            let before = !none && adv & (1 << 30) != 0;
            let page = !none && adv & (1 << 31) != 0;
            let n = if none { 1 } else { adv & 0xFFFF };
            // trailing spaces are not part of a line
            let mut end = rec.len();
            while end > 0 && rec[end - 1] == b' ' {
                end -= 1;
            }
            let body = &rec[..end];
            let breaks = |k: u32| -> Vec<u8> { if page { vec![0x0c, b'\n'] } else { vec![b'\n'; k as usize] } };
            if none {
                text.extend_from_slice(body);
                text.push(b'\n');
            } else if before {
                text.extend_from_slice(body);
                text.extend(breaks(n));
            } else {
                text.extend(breaks(n));
                text.extend_from_slice(body);
            }
            match caller.data_mut().files.get_mut(&(handle as u32)) {
                Some(OpenFile::Writer(f)) => {
                    if f.write_all(&text).is_err() {
                        return -30;
                    }
                    0
                }
                _ => -48,
            }
        })
        .map_err(|e| e.to_string())?;
    linker
        .func_wrap("pr", "file_read", |mut caller: Caller<'_, State>, handle: i32, cap: i32| -> i32 {
            let mut line = Vec::new();
            if handle == 0 {
                // The operator: one line of standard input.
                let mut s = String::new();
                match std::io::stdin().read_line(&mut s) {
                    Ok(0) | Err(_) => return -10,
                    Ok(_) => {}
                }
                line = s.trim_end_matches(['\n', '\r']).as_bytes().to_vec();
                line.truncate(cap as usize);
                let st = caller.data();
                let mem = st.arena.expect("arena");
                let at = st.arena_base + st.answer_at;
                mem.write(&mut caller, at as usize, &line).expect("write arena");
                return line.len() as i32;
            }
            match caller.data_mut().files.get_mut(&(handle as u32)) {
                Some(OpenFile::Reader(r)) => match r.read_until(b'\n', &mut line) {
                    Ok(0) => return -10,
                    Ok(_) => {}
                    Err(_) => return -30,
                },
                _ => return -47,
            }
            while line.last() == Some(&b'\n') || line.last() == Some(&b'\r') {
                line.pop();
            }
            line.truncate(cap as usize);
            let st = caller.data();
            let mem = st.arena.expect("arena");
            let at = st.arena_base + st.answer_at;
            mem.write(&mut caller, at as usize, &line).expect("write arena");
            line.len() as i32
        })
        .map_err(|e| e.to_string())?;
    linker
        .func_wrap("pr", "now", |caller: Caller<'_, State>| -> f64 {
            if let Some(t) = caller.data().now_ms {
                return t;
            }
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64() * 1000.0)
                .unwrap_or(0.0)
        })
        .map_err(|e| e.to_string())?;
    if want("query") {
        linker
            .func_wrap("pr", "query", |mut caller: Caller<'_, State>, len: i32| -> i32 {
                let batch = read_batch(&caller, len as u32);
                let Ok(cmds) = decode_batch(&batch) else { return -2 };
                let Some(query) = cmds.last().filter(|q| q.is_query()).cloned() else { return -2 };
                // The commands before the query are applied first (§6).
                let mut r = cobolt_wasm_abi::batch::BatchReader::new(&batch);
                for _ in 1..cmds.len() {
                    let _ = r.next();
                }
                let prefix = r.position();
                let out = caller.data().out.clone();
                {
                    let mut o = out.borrow_mut();
                    o.queries += 1;
                    apply(&batch[..prefix], &mut o);
                }
                remember_sets(&batch[..prefix], &mut caller.data_mut().props);
                let answer = match &query {
                    Command::Get(p) => {
                        let segs: Vec<u32> = p.segs.iter().map(|s| s.name).collect();
                        prop_key(p.inst, p.ctrl, &segs).and_then(|k| caller.data().props.get(&k).cloned()).unwrap_or_default()
                    }
                    Command::Count(_) => "0".to_string(),
                    _ => String::new(),
                };
                let st = caller.data();
                let mem = st.arena.expect("arena");
                let at = st.arena_base + st.answer_at;
                mem.write(&mut caller, at as usize, answer.as_bytes()).expect("write arena");
                answer.len() as i32
            })
            .map_err(|e| e.to_string())?;
    }
    if want("random") {
        linker.func_wrap("pr", "random", |caller: Caller<'_, State>| -> f64 { caller.data().random }).map_err(|e| e.to_string())?;
    }
    if want("remote") {
        linker
            .func_wrap("pr", "remote", |mut caller: Caller<'_, State>, len: i32| -> i32 {
                let st = caller.data();
                let mem = st.arena.expect("arena");
                // frame-out follows the batch and the answer regions (§2).
                let at = st.arena_base + 2 * st.answer_at;
                let mut buf = vec![0u8; len.max(0) as usize];
                if mem.read(&caller, at as usize, &mut buf).is_err() {
                    return -5;
                }
                let st = caller.data_mut();
                st.remote_frames.push(buf);
                (st.remote_frames.len() - 1) as i32
            })
            .map_err(|e| e.to_string())?;
    }
    Ok(linker)
}

fn run_with(wasm: &[u8], conformance: Option<Conformance>, batch_size: u32, keep_memory: bool) -> Result<(Outcome, Vec<u8>), String> {
    let engine = engine()?;
    let module = Module::new(&engine, wasm).map_err(|e| format!("invalid module: {e}"))?;
    let out = Rc::new(RefCell::new(Outcome::default()));
    let mut store = Store::new(&engine, new_state(out.clone(), conformance, batch_size));
    let linker = linker(&engine, &[])?;
    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|e| format!("cannot instantiate: {e}"))?;
    let Some(arena) = instance.get_memory(&mut store, "arena") else {
        return Err("no arena export".into());
    };
    let base_fn = instance.get_typed_func::<(), i32>(&mut store, "pr_arena_base").map_err(|e| e.to_string())?;
    let base = base_fn.call(&mut store, ()).map_err(|e| e.to_string())?;
    store.data_mut().arena = Some(arena);
    store.data_mut().arena_base = base as u32;
    let abi = instance.get_typed_func::<(), i32>(&mut store, "pr_abi").map_err(|e| e.to_string())?;
    if abi.call(&mut store, ()).map_err(|e| e.to_string())? != 1 {
        return Err("ABI version mismatch".into());
    }
    let run = instance.get_typed_func::<(), i32>(&mut store, "pr_run").map_err(|e| e.to_string())?;
    let result = run.call(&mut store, ());
    // Flush what is still buffered in files.
    let handles: Vec<u32> = store.data().files.keys().copied().collect();
    for h in handles {
        if let Some(OpenFile::Writer(mut f)) = store.data_mut().files.remove(&h) {
            let _ = f.flush();
        }
    }
    let mut outcome = out.borrow().clone();
    match result {
        Ok(s) => outcome.status = s,
        Err(e) => {
            outcome.status = -10;
            outcome.trapped = Some(format!("{e:#}"));
        }
    }
    let memory = match (keep_memory, instance.get_memory(&mut store, "memory")) {
        (true, Some(m)) => m.data(&store).to_vec(),
        _ => Vec::new(),
    };
    Ok((outcome, memory))
}

// ── driving a project module's forms (ABI v1 §4) ────────────────────────────

/// What a [`FormHost`] answers and lacks.
#[derive(Debug, Clone, Default)]
pub struct HostOptions {
    /// Imports this host does not provide (a module needing one cannot start).
    pub omit_imports: Vec<String>,
    /// A fixed clock (milliseconds since the epoch) instead of the real one.
    pub now_ms: Option<f64>,
    /// What `random` answers (default 0.5).
    pub random: Option<f64>,
    /// Keep every batch the module flushes ([`FormHost::batches`]).
    pub record_batches: bool,
}

/// A host for a project module: it opens forms, delivers events, resumes and
/// cancels tasks, and keeps the property values the module has set — the
/// smallest stand-in for the player that the tests and the Build's start-up
/// measurement need. Events raised while an instance is busy are queued and
/// delivered, in order, once it is idle (ABI §4, A5).
pub struct FormHost {
    store: Store<State>,
    instance: Instance,
    out: Rc<RefCell<Outcome>>,
    queue: VecDeque<(i32, i32, i32, Vec<String>)>,
    frame_size: u32,
}

impl FormHost {
    pub fn new(wasm: &[u8]) -> Result<FormHost, String> {
        FormHost::with(wasm, HostOptions::default())
    }

    /// Instantiate, check the ABI and run `pr_init`.
    pub fn with(wasm: &[u8], opts: HostOptions) -> Result<FormHost, String> {
        let scan = cobolt_wasm_abi::scan::scan_module(wasm)?;
        let manifest = scan.embedded_manifest()?;
        let engine = engine()?;
        let module = Module::new(&engine, wasm).map_err(|e| format!("invalid module: {e}"))?;
        let out = Rc::new(RefCell::new(Outcome::default()));
        let mut state = new_state(out.clone(), None, manifest.limits.batch_size);
        state.now_ms = opts.now_ms;
        if opts.record_batches {
            state.batches = Some(Vec::new());
        }
        if let Some(r) = opts.random {
            state.random = r;
        }
        let mut store = Store::new(&engine, state);
        let omit: Vec<&str> = opts.omit_imports.iter().map(String::as_str).collect();
        let linker = linker(&engine, &omit)?;
        let instance = linker.instantiate(&mut store, &module).map_err(|e| format!("cannot instantiate: {e:#}"))?;
        let Some(arena) = instance.get_memory(&mut store, "arena") else {
            return Err("no arena export".into());
        };
        let mut h = FormHost { store, instance, out, queue: VecDeque::new(), frame_size: manifest.limits.frame_size };
        let base = h.call0("pr_arena_base")?;
        h.store.data_mut().arena = Some(arena);
        h.store.data_mut().arena_base = base as u32;
        if h.call0("pr_abi")? != 1 {
            return Err("ABI version mismatch".into());
        }
        h.call0("pr_init")?;
        Ok(h)
    }

    fn call0(&mut self, name: &str) -> Result<i32, String> {
        let f = self.instance.get_typed_func::<(), i32>(&mut self.store, name).map_err(|e| e.to_string())?;
        f.call(&mut self.store, ()).map_err(|e| format!("{e:#}"))
    }

    /// A call into the module; a trap is reported as status `-10` and the run
    /// is discarded with `pr_cancel(-1)` (A5), so the application keeps running.
    fn run<P: wasmtime::WasmParams>(&mut self, name: &str, args: P) -> i32 {
        let f = match self.instance.get_typed_func::<P, i32>(&mut self.store, name) {
            Ok(f) => f,
            Err(e) => {
                self.out.borrow_mut().trapped = Some(e.to_string());
                return -10;
            }
        };
        match f.call(&mut self.store, args) {
            Ok(s) => s,
            Err(e) => {
                self.out.borrow_mut().trapped = Some(format!("{e:#}"));
                self.cancel(-1);
                -10
            }
        }
    }

    /// `pr_open_form(form, opener)`: the instance id (or a negative error) and
    /// the status of the start-up run.
    pub fn open_form(&mut self, form: i32, opener: i32) -> (i32, i32) {
        let inst = self.run("pr_open_form", (form, opener));
        let status = if inst == -10 { -10 } else { self.status() };
        (inst, status)
    }

    /// `pr_event(inst, ctrl, event, argc)` with the arguments in the answer region.
    pub fn event(&mut self, inst: i32, ctrl: i32, event: i32, args: &[&str]) -> i32 {
        let bytes = cobolt_wasm_abi::batch::encode_event_args(args);
        let st = self.store.data();
        let (mem, at) = (st.arena.expect("arena"), st.arena_base + st.answer_at);
        if mem.write(&mut self.store, at as usize, &bytes).is_err() {
            return -5;
        }
        self.run("pr_event", (inst, ctrl, event, args.len() as i32))
    }

    /// Raise an event as the page would: delivered now when no task of the
    /// module is live, queued otherwise (and delivered by [`FormHost::pump`]).
    pub fn post(&mut self, inst: i32, ctrl: i32, event: i32, args: &[&str]) -> Option<i32> {
        self.queue.push_back((inst, ctrl, event, args.iter().map(|a| a.to_string()).collect()));
        self.pump()
    }

    /// Deliver queued events while the module is idle; the last status.
    pub fn pump(&mut self) -> Option<i32> {
        let mut last = None;
        while self.tick() < 0 || self.status() == 0 || self.status() < 0 {
            let Some((i, c, e, a)) = self.queue.pop_front() else { break };
            let args: Vec<&str> = a.iter().map(String::as_str).collect();
            let s = self.event(i, c, e, &args);
            last = Some(s);
            if s == 1 || s == 2 {
                break;
            }
        }
        last
    }

    /// Events still waiting for the module.
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// `pr_resume(task, len)`, with `frame` (a remote answer) in frame-in.
    pub fn resume(&mut self, task: i32, frame: &[u8]) -> i32 {
        let st = self.store.data();
        // frame-in follows batch, answer and frame-out (§2).
        let (mem, base, answer) = (st.arena.expect("arena"), st.arena_base, st.answer_at);
        let at = base + 2 * answer + self.frame_size;
        if !frame.is_empty() && mem.write(&mut self.store, at as usize, frame).is_err() {
            return -5;
        }
        self.run("pr_resume", (task, frame.len() as i32))
    }

    pub fn cancel(&mut self, task: i32) {
        if let Ok(f) = self.instance.get_typed_func::<i32, ()>(&mut self.store, "pr_cancel") {
            let _ = f.call(&mut self.store, task);
        }
    }

    pub fn close_form(&mut self, inst: i32) -> i32 {
        self.run("pr_close_form", inst)
    }

    pub fn tick(&mut self) -> i32 {
        self.call0("pr_tick").unwrap_or(-1)
    }

    pub fn status(&mut self) -> i32 {
        self.call0("pr_status").unwrap_or(-10)
    }

    pub fn dbg_live(&mut self) -> i32 {
        self.call0("pr_dbg_live").unwrap_or(-1)
    }

    pub fn dbg_region_used(&mut self) -> i32 {
        self.call0("pr_dbg_region_used").unwrap_or(-1)
    }

    /// What the module has done so far.
    pub fn outcome(&self) -> Outcome {
        self.out.borrow().clone()
    }

    /// The property value the module last set (`SET inst ctrl name`).
    pub fn property(&self, inst: u32, ctrl: u32, name: u32) -> Option<String> {
        self.store.data().props.get(&(inst, ctrl, name)).cloned()
    }

    /// The batches the module flushed, byte for byte (with
    /// [`HostOptions::record_batches`]).
    pub fn batches(&self) -> Vec<Vec<u8>> {
        self.store.data().batches.clone().unwrap_or_default()
    }

    /// The request frames the module sent with `remote`.
    pub fn remote_frames(&self) -> Vec<Vec<u8>> {
        self.store.data().remote_frames.clone()
    }
}
