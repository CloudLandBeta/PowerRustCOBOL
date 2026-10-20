// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The mini-linker: take the routine blob (`assets/rt.wasm`, compiled from
//! `cobolt-wasm-rt`) and copy into the module being built exactly the
//! functions the generated code reaches (WASM-010 R2: "no routine it does not
//! use").
//!
//! The blob is a complete linked module, so linking is *tree-shaking plus
//! renumbering*: find the functions reachable from the entry points the
//! generated code calls, re-encode each body with its function, type and
//! global indices mapped into the new module, and stub the panic machinery of
//! `core` (formatting a message needs `core::fmt`, which would drag in
//! indirect calls and tens of kilobytes the module can never use: a panic in a
//! routine is a trap, and the host already reports it).
//!
//! The blob's static data stays **at the addresses it was linked for** — its
//! code embeds them — so the generator places its own data above
//! [`Blob::data_end`].

use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use wasm_encoder::reencode::{self, Reencode};
use wasmparser::{FuncType, Operator, Parser, Payload, ValType};

/// Substrings of function names whose bodies are replaced by `unreachable`.
const STUB_PATTERNS: &[&str] = &[
    "9panicking",           // core::panicking::*
    "slice_index_fail",
    "slice_start_index",
    "slice_end_index",
    "begin_unwind",
    "core3fmt",             // core::fmt::*
    "4core3fmt",
    "do_count_chars",
    "unwrap_failed",
    "expect_failed",
    "5alloc",               // alloc::* (nothing should reach it)
];

#[derive(Debug)]
pub struct LinkError(pub String);

impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LinkError {}

fn err<T>(m: impl Into<String>) -> Result<T, LinkError> {
    Err(LinkError(m.into()))
}

pub struct Blob {
    bytes: Vec<u8>,
    pub types: Vec<FuncType>,
    /// `(module, name, type index)` of the function imports.
    pub imports: Vec<(String, String, u32)>,
    /// Type index of each defined function.
    func_types: Vec<u32>,
    /// Byte range of each defined function's body in `bytes`.
    bodies: Vec<Range<usize>>,
    names: HashMap<u32, String>,
    exports: HashMap<String, u32>,
    /// Active data segments `(address, bytes)`.
    pub data: Vec<(u32, Vec<u8>)>,
    pub stack_top: u32,
    pub data_end: u32,
}

impl Blob {
    pub fn load(bytes: &[u8]) -> Result<Blob, LinkError> {
        let mut b = Blob {
            bytes: bytes.to_vec(),
            types: vec![],
            imports: vec![],
            func_types: vec![],
            bodies: vec![],
            names: HashMap::new(),
            exports: HashMap::new(),
            data: vec![],
            stack_top: 0,
            data_end: 0,
        };
        let mut globals: Vec<(bool, i64)> = Vec::new();
        let mut global_exports: HashMap<String, u32> = HashMap::new();
        for payload in Parser::new(0).parse_all(bytes) {
            let payload = payload.map_err(|e| LinkError(format!("rt blob: {e}")))?;
            match payload {
                Payload::TypeSection(r) => {
                    for ty in r.into_iter_err_on_gc_types() {
                        b.types.push(ty.map_err(|e| LinkError(format!("rt blob type: {e}")))?);
                    }
                }
                Payload::ImportSection(r) => {
                    for imp in r.into_imports() {
                        let imp = imp.map_err(|e| LinkError(e.to_string()))?;
                        match imp.ty {
                            wasmparser::TypeRef::Func(t) => b.imports.push((imp.module.to_string(), imp.name.to_string(), t)),
                            _ => return err("rt blob imports something other than functions"),
                        }
                    }
                }
                Payload::FunctionSection(r) => {
                    for t in r {
                        b.func_types.push(t.map_err(|e| LinkError(e.to_string()))?);
                    }
                }
                Payload::GlobalSection(r) => {
                    for g in r {
                        let g = g.map_err(|e| LinkError(e.to_string()))?;
                        let mut ops = g.init_expr.get_operators_reader();
                        let v = match ops.read().map_err(|e| LinkError(e.to_string()))? {
                            Operator::I32Const { value } => value as i64,
                            _ => 0,
                        };
                        globals.push((g.ty.mutable, v));
                    }
                }
                Payload::ExportSection(r) => {
                    for e in r {
                        let e = e.map_err(|e| LinkError(e.to_string()))?;
                        match e.kind {
                            wasmparser::ExternalKind::Func => {
                                b.exports.insert(e.name.to_string(), e.index);
                            }
                            wasmparser::ExternalKind::Global => {
                                global_exports.insert(e.name.to_string(), e.index);
                            }
                            _ => {}
                        }
                    }
                }
                Payload::CodeSectionEntry(body) => {
                    b.bodies.push(body.range());
                }
                Payload::DataSection(r) => {
                    for d in r {
                        let d = d.map_err(|e| LinkError(e.to_string()))?;
                        if let wasmparser::DataKind::Active { memory_index: 0, offset_expr } = d.kind {
                            let mut ops = offset_expr.get_operators_reader();
                            if let Operator::I32Const { value } = ops.read().map_err(|e| LinkError(e.to_string()))? {
                                b.data.push((value as u32, d.data.to_vec()));
                            }
                        }
                    }
                }
                Payload::CustomSection(c) => {
                    if let wasmparser::KnownCustom::Name(reader) = c.as_known() {
                        for n in reader {
                            if let Ok(wasmparser::Name::Function(map)) = n {
                                for naming in map.into_iter().flatten() {
                                    b.names.insert(naming.index, naming.name.to_string());
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        b.stack_top = globals.first().map(|g| g.1 as u32).unwrap_or(0);
        b.data_end = global_exports
            .get("__data_end")
            .and_then(|i| globals.get(*i as usize))
            .map(|g| g.1 as u32)
            .ok_or_else(|| LinkError("rt blob has no __data_end".into()))?;
        Ok(b)
    }

    pub fn export(&self, name: &str) -> Option<u32> {
        self.exports.get(name).copied()
    }

    fn n_imports(&self) -> u32 {
        self.imports.len() as u32
    }

    pub fn name_of(&self, idx: u32) -> String {
        self.names.get(&idx).cloned().unwrap_or_else(|| format!("rt-func-{idx}"))
    }

    fn is_stub(&self, idx: u32) -> bool {
        let n = self.name_of(idx);
        if n.contains("cobolt_wasm_rt4fail") {
            return false; // our own error exit: it calls the glue abort
        }
        STUB_PATTERNS.iter().any(|p| n.contains(p))
    }

    fn body(&self, idx: u32) -> wasmparser::FunctionBody<'_> {
        let r = self.bodies[(idx - self.n_imports()) as usize].clone();
        wasmparser::FunctionBody::new(wasmparser::BinaryReader::new(&self.bytes[r.clone()], r.start))
    }

    /// The functions called directly by body `idx`.
    fn callees(&self, idx: u32) -> Result<Vec<u32>, LinkError> {
        let body = self.body(idx);
        let mut out = Vec::new();
        let mut ops = body.get_operators_reader().map_err(|e| LinkError(e.to_string()))?;
        while !ops.eof() {
            match ops.read().map_err(|e| LinkError(e.to_string()))? {
                Operator::Call { function_index } => out.push(function_index),
                Operator::CallIndirect { .. } | Operator::ReturnCallIndirect { .. } => {
                    return err(format!(
                        "rt function {} uses an indirect call; the linker does not support a table",
                        self.name_of(idx)
                    ))
                }
                Operator::MemoryGrow { .. } => {
                    return err(format!("rt function {} grows memory (WASM-010 R3 forbids it)", self.name_of(idx)))
                }
                _ => {}
            }
        }
        Ok(out)
    }

    /// Everything reachable from the exported routines in `roots`.
    pub fn closure(&self, roots: &[&str]) -> Result<Closure, LinkError> {
        let mut funcs: BTreeSet<u32> = BTreeSet::new();
        let mut imports: BTreeSet<u32> = BTreeSet::new();
        let mut stack: Vec<u32> = Vec::new();
        for r in roots {
            match self.export(r) {
                Some(i) => stack.push(i),
                None => return err(format!("the routine blob has no export {r}")),
            }
        }
        while let Some(i) = stack.pop() {
            if i < self.n_imports() {
                imports.insert(i);
                continue;
            }
            if !funcs.insert(i) {
                continue;
            }
            if self.is_stub(i) {
                continue;
            }
            for c in self.callees(i)? {
                stack.push(c);
            }
        }
        Ok(Closure { funcs: funcs.into_iter().collect(), imports: imports.into_iter().collect() })
    }
}

/// The functions to copy.
#[derive(Debug, Clone)]
pub struct Closure {
    /// Defined functions, ascending blob index.
    pub funcs: Vec<u32>,
    /// Imported (glue) functions used, by blob index.
    pub imports: Vec<u32>,
}

/// Maps the blob's indices into the module being built.
pub struct Mapper<'m> {
    pub func: &'m HashMap<u32, u32>,
    pub types: &'m HashMap<u32, u32>,
    pub stack_global: u32,
}

impl Reencode for Mapper<'_> {
    type Error = LinkError;

    fn function_index(&mut self, f: u32) -> Result<u32, reencode::Error<LinkError>> {
        self.func
            .get(&f)
            .copied()
            .ok_or_else(|| reencode::Error::UserError(LinkError(format!("function {f} was not linked"))))
    }

    fn type_index(&mut self, t: u32) -> Result<u32, reencode::Error<LinkError>> {
        self.types
            .get(&t)
            .copied()
            .ok_or_else(|| reencode::Error::UserError(LinkError(format!("type {t} was not mapped"))))
    }

    fn global_index(&mut self, g: u32) -> Result<u32, reencode::Error<LinkError>> {
        if g == 0 {
            Ok(self.stack_global)
        } else {
            Err(reencode::Error::UserError(LinkError(format!("a routine reads global {g}; only the stack pointer is linked"))))
        }
    }

    fn memory_index(&mut self, m: u32) -> Result<u32, reencode::Error<LinkError>> {
        if m == 0 {
            Ok(0)
        } else {
            Err(reencode::Error::UserError(LinkError("a routine addresses a second memory".into())))
        }
    }
}

impl Blob {
    /// Re-encode body `idx` into `code`, with indices mapped; a stubbed
    /// function becomes `unreachable`.
    pub fn emit_body(
        &self,
        idx: u32,
        map: &mut Mapper<'_>,
        code: &mut wasm_encoder::CodeSection,
    ) -> Result<(), LinkError> {
        if self.is_stub(idx) {
            let mut f = wasm_encoder::Function::new([]);
            f.instruction(&wasm_encoder::Instruction::Unreachable);
            f.instruction(&wasm_encoder::Instruction::End);
            code.function(&f);
            return Ok(());
        }
        map.parse_function_body(code, self.body(idx))
            .map_err(|e| LinkError(format!("re-encoding {}: {e:?}", self.name_of(idx))))
    }

    /// Type of a function (imported or defined), as an index into `types`.
    pub fn type_of(&self, idx: u32) -> u32 {
        if idx < self.n_imports() {
            self.imports[idx as usize].2
        } else {
            self.func_types[(idx - self.n_imports()) as usize]
        }
    }

    pub fn func_type(&self, ty: u32) -> (Vec<ValType>, Vec<ValType>) {
        let t = &self.types[ty as usize];
        (t.params().to_vec(), t.results().to_vec())
    }

    pub fn import_name(&self, idx: u32) -> (&str, &str) {
        let i = &self.imports[idx as usize];
        (&i.0, &i.1)
    }
}
