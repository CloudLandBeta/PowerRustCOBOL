// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A host for generated modules, over `wasmi`: it implements ABI v1 for the
//! tests and for the NIST gate. It is *not* the browser host (that is the
//! player's) — it is the smallest thing that can stand in for one, which is
//! what makes the conformance numbers about the module and not about a page.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::rc::Rc;
use wasmi::{Caller, Engine, Extern, Linker, Memory, Module, Store};

/// Where a conformance run finds and creates its files.
#[derive(Debug, Clone)]
pub struct Conformance {
    pub cwd: PathBuf,
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
}

struct State {
    arena: Option<Memory>,
    arena_base: u32,
    answer_at: u32,
    out: Rc<RefCell<Outcome>>,
    conformance: Option<Conformance>,
    files: HashMap<u32, OpenFile>,
    next_handle: u32,
}

fn varint(b: &[u8], i: &mut usize) -> u32 {
    let mut v: u32 = 0;
    let mut shift = 0;
    loop {
        let c = b[*i];
        *i += 1;
        v |= ((c & 0x7F) as u32) << shift;
        if c & 0x80 == 0 {
            return v;
        }
        shift += 7;
    }
}

/// Apply a command batch to the outcome.
fn apply(batch: &[u8], out: &mut Outcome) {
    let mut i = 0;
    while i < batch.len() {
        let op = batch[i];
        i += 1;
        match op {
            0x03 => {
                let n = varint(batch, &mut i) as usize;
                out.output.push_str(&String::from_utf8_lossy(&batch[i..i + n]));
                i += n;
            }
            0x04 => {
                let code = varint(batch, &mut i);
                let n = varint(batch, &mut i) as usize;
                i += n;
                let _unit = varint(batch, &mut i);
                let line = varint(batch, &mut i);
                let _form = varint(batch, &mut i);
                let _event = varint(batch, &mut i);
                out.errors.push(RtError { code, line });
            }
            other => {
                out.errors.push(RtError { code: 9000 + other as u32, line: 0 });
                return;
            }
        }
    }
}

fn read_batch(caller: &Caller<'_, State>, len: u32) -> Vec<u8> {
    let st = caller.data();
    let mem = st.arena.expect("arena");
    let mut buf = vec![0u8; len as usize];
    mem.read(caller, st.arena_base as usize, &mut buf).expect("read arena");
    buf
}

/// Run a program-mode module (`pr_run`) to its end.
pub fn run_program(wasm: &[u8]) -> Result<Outcome, String> {
    run_with(wasm, None, 64 * 1024)
}

/// Run a conformance module: file verbs work against `conf.cwd`.
pub fn run_conformance(wasm: &[u8], conf: Conformance) -> Result<Outcome, String> {
    run_with(wasm, Some(conf), 64 * 1024)
}

fn run_with(wasm: &[u8], conformance: Option<Conformance>, batch_size: u32) -> Result<Outcome, String> {
    let mut config = wasmi::Config::default();
    config.wasm_multi_memory(true);
    let engine = Engine::new(&config);
    let module = Module::new(&engine, wasm).map_err(|e| format!("invalid module: {e}"))?;
    let out = Rc::new(RefCell::new(Outcome::default()));
    let mut store = Store::new(
        &engine,
        State {
            arena: None,
            arena_base: 0,
            answer_at: batch_size,
            out: out.clone(),
            conformance,
            files: HashMap::new(),
            next_handle: 1,
        },
    );
    let mut linker: Linker<State> = Linker::new(&engine);
    linker
        .func_wrap("pr", "flush", |caller: Caller<'_, State>, len: i32| {
            let batch = read_batch(&caller, len as u32);
            let out = caller.data().out.clone();
            let mut o = out.borrow_mut();
            o.flushes += 1;
            apply(&batch, &mut o);
        })
        .map_err(|e| e.to_string())?;
    linker
        .func_wrap("pr", "abort", |caller: Caller<'_, State>, len: i32| -> Result<(), wasmi::Error> {
            let batch = read_batch(&caller, len as u32);
            let out = caller.data().out.clone();
            apply(&batch, &mut out.borrow_mut());
            Err(wasmi::Error::new("run-time error"))
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
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .map_err(|e| format!("cannot instantiate: {e}"))?;
    let arena = match instance.get_export(&store, "arena") {
        Some(Extern::Memory(m)) => m,
        _ => return Err("no arena export".into()),
    };
    let base_fn = instance.get_typed_func::<(), i32>(&store, "pr_arena_base").map_err(|e| e.to_string())?;
    let base = base_fn.call(&mut store, ()).map_err(|e| e.to_string())?;
    store.data_mut().arena = Some(arena);
    store.data_mut().arena_base = base as u32;
    let abi = instance.get_typed_func::<(), i32>(&store, "pr_abi").map_err(|e| e.to_string())?;
    if abi.call(&mut store, ()).map_err(|e| e.to_string())? != 1 {
        return Err("ABI version mismatch".into());
    }
    let run = instance.get_typed_func::<(), i32>(&store, "pr_run").map_err(|e| e.to_string())?;
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
            outcome.trapped = Some(e.to_string());
        }
    }
    Ok(outcome)
}
