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
use std::rc::Rc;
use wasmi::{Caller, Engine, Extern, Linker, Memory, Module, Store};

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
    out: Rc<RefCell<Outcome>>,
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
    let mut config = wasmi::Config::default();
    config.wasm_multi_memory(true);
    let engine = Engine::new(&config);
    let module = Module::new(&engine, wasm).map_err(|e| format!("invalid module: {e}"))?;
    let out = Rc::new(RefCell::new(Outcome::default()));
    let mut store = Store::new(&engine, State { arena: None, arena_base: 0, out: out.clone() });
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
