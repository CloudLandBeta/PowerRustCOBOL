// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Assembling the module: memory map, linked routines, block functions, the
//! dispatcher, the ABI exports and the custom sections (WASM-ABI-v1).

use crate::error::{GenError, GenResult};
use crate::image::{self, InitOp};
use crate::ir::*;
use crate::link::{Blob, Mapper};
use crate::lower::{Lower, MemMap};
use crate::pool::Pool;
use cobolt_ast::program::Program;
use cobolt_wasm_rt::state::WS_SIZE;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, CustomSection, DataSection, ElementSection, Elements, EntityType, ExportKind,
    ExportSection, Function, FunctionSection, GlobalSection, GlobalType, ImportSection, Instruction as I, MemArg,
    MemoryType, Module, NameMap, NameSection, RefType, TableSection, TableType, TypeSection, ValType,
};

static RT_WASM: &[u8] = include_bytes!("../assets/rt.wasm");

/// The ABI version this generator produces (WASM-ABI-v1).
pub const ABI_VERSION: i32 = 1;

/// Sizes of the regions of the arena (ABI v1 §2).
#[derive(Debug, Clone, Copy)]
pub struct Arena {
    pub batch: u32,
    pub answer: u32,
    pub frame: u32,
}

impl Arena {
    pub fn total(&self) -> u32 {
        self.batch + self.answer + 2 * self.frame
    }
}

impl Default for Arena {
    fn default() -> Self {
        Arena { batch: 64 * 1024, answer: 64 * 1024, frame: 64 * 1024 }
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    /// Two memories (arena in memory 1) or one.
    pub multi_memory: bool,
    /// The NIST gate build: accepts the file verbs through host imports.
    pub conformance: bool,
    pub arena: Arena,
    /// Blocks run before the module yields to the host.
    pub budget: i32,
    pub name: String,
}

impl Default for Options {
    fn default() -> Self {
        Options { multi_memory: true, conformance: false, arena: Arena::default(), budget: 20_000, name: "program".into() }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Sizes {
    pub module_bytes: usize,
    pub rt_functions: usize,
    pub rt_bytes: usize,
    pub blocks: usize,
    pub data_pages: u32,
}

pub struct Compiled {
    pub wasm: Vec<u8>,
    pub manifest: serde_json::Value,
    pub sizes: Sizes,
}

struct Types {
    list: Vec<(Vec<ValType>, Vec<ValType>)>,
}

impl Types {
    fn intern(&mut self, p: &[ValType], r: &[ValType]) -> u32 {
        for (i, (a, b)) in self.list.iter().enumerate() {
            if a == p && b == r {
                return i as u32;
            }
        }
        self.list.push((p.to_vec(), r.to_vec()));
        (self.list.len() - 1) as u32
    }
}

fn wp(v: &wasmparser::ValType) -> ValType {
    match v {
        wasmparser::ValType::I32 => ValType::I32,
        wasmparser::ValType::I64 => ValType::I64,
        wasmparser::ValType::F32 => ValType::F32,
        wasmparser::ValType::F64 => ValType::F64,
        _ => ValType::I32,
    }
}

const fn align(n: u32, a: u32) -> u32 {
    (n + a - 1) & !(a - 1)
}

fn mem0(offset: u64, align: u32) -> MemArg {
    MemArg { offset, align, memory_index: 0 }
}

enum Body {
    Rt(u32),
    Encoded(Function),
}

struct Def {
    name: String,
    ty: u32,
    body: Body,
}

pub fn compile_program(prog: &Program, opts: &Options) -> GenResult<Compiled> {
    let blob = Blob::load(RT_WASM).map_err(|e| GenError::new(0, e.to_string()))?;

    // ── memory map ───────────────────────────────────────────────────────
    let rt_ws = align(blob.data_end, 16);
    let pstack_base = align(rt_ws + WS_SIZE, 16);
    let pstack_end = pstack_base + 4096;
    let arena_total = opts.arena.total();
    let (arena_base, consts_base) = if opts.multi_memory {
        (0, align(pstack_end, 16))
    } else {
        let a = align(pstack_end, 16);
        (a, align(a + arena_total, 16))
    };
    let map = MemMap { pstack_base, pstack_end };

    // ── lowering ─────────────────────────────────────────────────────────
    let mut pool = Pool::new(consts_base);
    let mut blocks: Vec<Block> = Vec::new();
    let lowered = Lower::lower(prog, &mut pool, &mut blocks, map, opts.conformance)?;
    let layout = lowered.layout;

    // Reachable blocks, entry first.
    let mut order: Vec<BlockId> = Vec::new();
    let mut seen: HashMap<BlockId, u32> = HashMap::new();
    let mut stack = vec![lowered.entry];
    while let Some(b) = stack.pop() {
        if seen.contains_key(&b) {
            continue;
        }
        seen.insert(b, order.len() as u32);
        order.push(b);
        // Every block a block can name — as a jump, a branch, a frame it
        // pushes — appears in its code as a block reference.
        for ins in blocks[b].code.iter().rev() {
            if let Ins::Pc(t) = ins {
                stack.push(*t);
            }
        }
    }
    for &b in &order {
        if !blocks[b].done {
            return Err(GenError::new(0, format!("internal error: block {} was never terminated", blocks[b].name)));
        }
    }

    // ── image and memory map, continued ──────────────────────────────────
    let img = image::build(&layout, prog.decimal_comma, prog.currency)?;
    let template_base = align(consts_base + pool.bytes.len() as u32, 16);
    let region_base = align(template_base + img.blob.len() as u32, 16);
    let region_size = layout.total;
    let end = region_base + region_size;
    let data_pages = end.div_ceil(65536).max(1);
    let arena_pages = arena_total.div_ceil(65536).max(1);

    // ── which routines ───────────────────────────────────────────────────
    let mut roots: BTreeSet<&'static str> = BTreeSet::new();
    roots.insert("rt_init");
    roots.insert("rt_flush");
    for &b in &order {
        for ins in &blocks[b].code {
            if let Ins::CallRt(n) = ins {
                roots.insert(*n);
            }
        }
    }
    let root_list: Vec<&str> = roots.iter().copied().collect();
    let closure = blob.closure(&root_list).map_err(|e| GenError::new(0, e.to_string()))?;

    // ── types ────────────────────────────────────────────────────────────
    let mut types = Types { list: Vec::new() };
    let mut blob_types: HashMap<u32, u32> = HashMap::new();
    for (i, t) in blob.types.iter().enumerate() {
        let p: Vec<ValType> = t.params().iter().map(wp).collect();
        let r: Vec<ValType> = t.results().iter().map(wp).collect();
        blob_types.insert(i as u32, types.intern(&p, &r));
    }
    let ty_block = types.intern(&[], &[ValType::I32]);
    let ty_i32_void = types.intern(&[ValType::I32], &[]);
    let ty_i32_i32_void = types.intern(&[ValType::I32, ValType::I32], &[]);
    let ty_i32_ret = types.intern(&[], &[ValType::I32]);

    // ── function index space ────────────────────────────────────────────
    // The glue functions the module needs: the two it always has, plus the
    // file ones a conformance build's routines import. Each is backed by a
    // host import of the same name in the `pr` namespace.
    const GLUE_ORDER: [&str; 6] = ["flush", "abort", "file_open", "file_close", "file_write", "file_read"];
    let mut glue_needed: BTreeSet<&'static str> = BTreeSet::new();
    glue_needed.insert("flush");
    glue_needed.insert("abort");
    for &i in &closure.imports {
        let (m, n) = blob.import_name(i);
        let name = GLUE_ORDER.iter().copied().find(|g| *g == n && m == "glue");
        match name {
            Some(g) => {
                if g.starts_with("file_") && !opts.conformance {
                    return Err(GenError::new(
                        0,
                        "file input/output is not available in a browser module: it runs on the backend; call a backend program with CALL … WITH WEBSOCKET LINKAGE",
                    ));
                }
                glue_needed.insert(g);
            }
            None => return Err(GenError::new(0, format!("the routine blob imports glue.{n}, which the generator does not provide"))),
        }
    }
    let glue_list: Vec<&'static str> = GLUE_ORDER.iter().copied().filter(|g| glue_needed.contains(g)).collect();
    let ty_i32_i32_i32_ret = types.intern(&[ValType::I32, ValType::I32, ValType::I32], &[ValType::I32]);
    let ty_i32_i32_i32_i32_ret = types.intern(&[ValType::I32; 4], &[ValType::I32]);
    let ty_i32_i32_ret = types.intern(&[ValType::I32, ValType::I32], &[ValType::I32]);
    let ty_i32_ret1 = types.intern(&[ValType::I32], &[ValType::I32]);
    // (host import type, glue function type)
    let glue_types = |name: &str| -> (u32, u32) {
        match name {
            "flush" => (ty_i32_void, ty_i32_i32_void),
            "abort" => (ty_i32_void, ty_i32_void),
            "file_open" => (ty_i32_i32_ret, ty_i32_i32_i32_ret),
            "file_close" => (ty_i32_ret1, ty_i32_ret1),
            "file_write" => (ty_i32_i32_i32_ret, ty_i32_i32_i32_i32_ret),
            _ => (ty_i32_i32_ret, ty_i32_i32_i32_ret), // file_read
        }
    };
    let n_imports = glue_list.len() as u32;
    let imp_of = |name: &str| -> u32 { glue_list.iter().position(|g| *g == name).unwrap() as u32 };
    let mut defs: Vec<Def> = Vec::new();
    let mut glue_final: HashMap<&'static str, u32> = HashMap::new();
    for g in &glue_list {
        let idx = n_imports + defs.len() as u32;
        glue_final.insert(*g, idx);
        defs.push(Def { name: format!("glue.{g}"), ty: glue_types(g).1, body: Body::Encoded(Function::new([])) });
    }
    let g_flush = glue_final["flush"];
    let g_abort = glue_final["abort"];
    let imp_flush = imp_of("flush");
    let imp_abort = imp_of("abort");

    let mut rt_final: HashMap<u32, u32> = HashMap::new();
    for &f in &closure.funcs {
        let idx = n_imports + defs.len() as u32;
        rt_final.insert(f, idx);
        let (p, r) = blob.func_type(blob.type_of(f));
        let ty = types.intern(&p.iter().map(wp).collect::<Vec<_>>(), &r.iter().map(wp).collect::<Vec<_>>());
        defs.push(Def { name: blob.name_of(f), ty, body: Body::Rt(f) });
    }
    // The blob's glue imports map to our glue functions.
    for &i in &closure.imports {
        let (_, n) = blob.import_name(i);
        rt_final.insert(i, glue_final[n]);
    }

    let block_fn_base = n_imports + defs.len() as u32;
    let mut table_funcs: Vec<u32> = Vec::new();
    // Globals: 0 stack pointer, 1 base, 2 psp, 3 fuel, 4 line, 5 pc.
    let gl = |g: G| -> u32 {
        match g {
            G::Base => 1,
            G::Psp => 2,
            G::Fuel => 3,
            G::Line => 4,
        }
    };
    let g_pc = 5u32;

    let mut rt_index_by_name: HashMap<&'static str, u32> = HashMap::new();
    for n in &roots {
        let bi = blob.export(n).ok_or_else(|| GenError::new(0, format!("the routine blob has no {n}")))?;
        rt_index_by_name.insert(*n, rt_final[&bi]);
    }

    for (k, &b) in order.iter().enumerate() {
        let block = &blocks[b];
        let mut f = Function::new([(2, ValType::I32)]);
        for ins in &block.code {
            match ins {
                Ins::Raw(i) => {
                    f.instruction(i);
                }
                Ins::CallRt(n) => {
                    f.instruction(&I::Call(rt_index_by_name[n]));
                }
                Ins::CallGlue(Glue::Flush) => {
                    f.instruction(&I::Call(g_flush));
                }
                Ins::CallGlue(Glue::Abort) => {
                    f.instruction(&I::Call(g_abort));
                }
                Ins::Pc(t) => {
                    f.instruction(&I::I32Const(seen[t] as i32));
                }
                Ins::GlobalGet(g) => {
                    f.instruction(&I::GlobalGet(gl(*g)));
                }
                Ins::GlobalSet(g) => {
                    f.instruction(&I::GlobalSet(gl(*g)));
                }
            }
        }
        f.instruction(&I::End);
        let idx = block_fn_base + k as u32;
        table_funcs.push(idx);
        defs.push(Def { name: format!("{}#{}", block.name, k), ty: ty_block, body: Body::Encoded(f) });
    }

    // ── glue bodies ──────────────────────────────────────────────────────
    let arena_mem = if opts.multi_memory { 1 } else { 0 };
    let arena_at = arena_base;
    {
        // glue.flush(addr, len): copy to the arena batch, call pr.flush(len)
        let mut f = Function::new([]);
        f.instruction(&I::I32Const(arena_at as i32));
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::LocalGet(1));
        f.instruction(&I::MemoryCopy { src_mem: 0, dst_mem: arena_mem });
        f.instruction(&I::LocalGet(1));
        f.instruction(&I::Call(imp_flush));
        f.instruction(&I::End);
        defs[(g_flush - n_imports) as usize].body = Body::Encoded(f);
    }
    let answer_at = arena_at + opts.arena.batch;
    if glue_final.contains_key("file_open") {
        // glue.file_open(addr, len, mode): the name goes to the arena batch.
        let mut f = Function::new([]);
        f.instruction(&I::I32Const(arena_at as i32));
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::LocalGet(1));
        f.instruction(&I::MemoryCopy { src_mem: 0, dst_mem: arena_mem });
        f.instruction(&I::LocalGet(1));
        f.instruction(&I::LocalGet(2));
        f.instruction(&I::Call(imp_of("file_open")));
        f.instruction(&I::End);
        defs[(glue_final["file_open"] - n_imports) as usize].body = Body::Encoded(f);
    }
    if glue_final.contains_key("file_close") {
        let mut f = Function::new([]);
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::Call(imp_of("file_close")));
        f.instruction(&I::End);
        defs[(glue_final["file_close"] - n_imports) as usize].body = Body::Encoded(f);
    }
    if glue_final.contains_key("file_write") {
        // glue.file_write(handle, addr, len, adv)
        let mut f = Function::new([]);
        f.instruction(&I::I32Const(arena_at as i32));
        f.instruction(&I::LocalGet(1));
        f.instruction(&I::LocalGet(2));
        f.instruction(&I::MemoryCopy { src_mem: 0, dst_mem: arena_mem });
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::LocalGet(2));
        f.instruction(&I::LocalGet(3));
        f.instruction(&I::Call(imp_of("file_write")));
        f.instruction(&I::End);
        defs[(glue_final["file_write"] - n_imports) as usize].body = Body::Encoded(f);
    }
    if glue_final.contains_key("file_read") {
        // glue.file_read(handle, addr, cap): the host leaves the record in the
        // arena answer area; copy min(n, cap) bytes back.
        let mut f = Function::new([(1, ValType::I32)]);
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::LocalGet(2));
        f.instruction(&I::Call(imp_of("file_read")));
        f.instruction(&I::LocalSet(3));
        f.instruction(&I::LocalGet(3));
        f.instruction(&I::I32Const(0));
        f.instruction(&I::I32GtS);
        f.instruction(&I::If(BlockType::Empty));
        f.instruction(&I::LocalGet(1));
        f.instruction(&I::I32Const(answer_at as i32));
        f.instruction(&I::LocalGet(3));
        f.instruction(&I::LocalGet(2));
        f.instruction(&I::LocalGet(3));
        f.instruction(&I::LocalGet(2));
        f.instruction(&I::I32LtU);
        f.instruction(&I::Select);
        f.instruction(&I::MemoryCopy { src_mem: arena_mem, dst_mem: 0 });
        f.instruction(&I::End);
        f.instruction(&I::LocalGet(3));
        f.instruction(&I::End);
        defs[(glue_final["file_read"] - n_imports) as usize].body = Body::Encoded(f);
    }
    {
        // glue.abort(code): ERROR command with the line, then pr.abort, then trap.
        let mut f = Function::new([]);
        let st = |f: &mut Function, off: u32, v: i32| {
            f.instruction(&I::I32Const((arena_at + off) as i32));
            f.instruction(&I::I32Const(v));
            f.instruction(&I::I32Store8(MemArg { offset: 0, align: 0, memory_index: arena_mem }));
        };
        st(&mut f, 0, 0x04);
        // code (a single varint byte: codes are below 128)
        f.instruction(&I::I32Const(arena_at as i32 + 1));
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::I32Store8(MemArg { offset: 0, align: 0, memory_index: arena_mem }));
        st(&mut f, 2, 0); // message length 0
        st(&mut f, 3, 0); // unit
        for k in 0..5u32 {
            // line as a 5-byte padded varint
            f.instruction(&I::I32Const((arena_at + 4 + k) as i32));
            f.instruction(&I::GlobalGet(4));
            f.instruction(&I::I32Const((7 * k) as i32));
            f.instruction(&I::I32ShrU);
            f.instruction(&I::I32Const(if k < 4 { 0x7F } else { 0x0F }));
            f.instruction(&I::I32And);
            if k < 4 {
                f.instruction(&I::I32Const(0x80));
                f.instruction(&I::I32Or);
            }
            f.instruction(&I::I32Store8(MemArg { offset: 0, align: 0, memory_index: arena_mem }));
        }
        st(&mut f, 9, 0); // form
        st(&mut f, 10, 0); // event
        f.instruction(&I::I32Const(11));
        f.instruction(&I::Call(imp_abort));
        f.instruction(&I::Unreachable);
        f.instruction(&I::End);
        defs[(g_abort - n_imports) as usize].body = Body::Encoded(f);
    }

    // ── dispatcher and exports ───────────────────────────────────────────
    let run_idx = n_imports + defs.len() as u32;
    {
        let mut f = Function::new([(1, ValType::I32)]);
        f.instruction(&I::Loop(BlockType::Empty));
        f.instruction(&I::GlobalGet(g_pc));
        f.instruction(&I::CallIndirect { type_index: ty_block, table_index: 0 });
        f.instruction(&I::LocalTee(0));
        f.instruction(&I::GlobalSet(g_pc));
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::I32Const(0));
        f.instruction(&I::I32LtS);
        f.instruction(&I::If(BlockType::Empty));
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::Return);
        f.instruction(&I::End);
        f.instruction(&I::GlobalGet(3));
        f.instruction(&I::I32Const(1));
        f.instruction(&I::I32Sub);
        f.instruction(&I::GlobalSet(3));
        f.instruction(&I::GlobalGet(3));
        f.instruction(&I::I32Const(0));
        f.instruction(&I::I32LtS);
        f.instruction(&I::If(BlockType::Empty));
        f.instruction(&I::I32Const(-2));
        f.instruction(&I::Return);
        f.instruction(&I::End);
        f.instruction(&I::Br(0));
        f.instruction(&I::End);
        f.instruction(&I::Unreachable);
        f.instruction(&I::End);
        defs.push(Def { name: "run".into(), ty: ty_i32_ret, body: Body::Encoded(f) });
    }
    let exp_const = |name: &str, v: i32, defs: &mut Vec<Def>, ty: u32| -> u32 {
        let idx = n_imports + defs.len() as u32;
        let mut f = Function::new([]);
        f.instruction(&I::I32Const(v));
        f.instruction(&I::End);
        defs.push(Def { name: name.into(), ty, body: Body::Encoded(f) });
        idx
    };
    let e_abi = exp_const("pr_abi", ABI_VERSION, &mut defs, ty_i32_ret);
    let variant = (opts.multi_memory as i32) | ((opts.conformance as i32) << 1);
    let e_variant = exp_const("pr_variant", variant, &mut defs, ty_i32_ret);
    let e_arena = exp_const("pr_arena_base", arena_base as i32, &mut defs, ty_i32_ret);

    let init_idx = n_imports + defs.len() as u32;
    {
        let mut f = Function::new([]);
        f.instruction(&I::I32Const(rt_ws as i32));
        f.instruction(&I::Call(rt_index_by_name["rt_init"]));
        f.instruction(&I::I32Const(pstack_base as i32));
        f.instruction(&I::GlobalSet(2));
        f.instruction(&I::I32Const(opts.budget));
        f.instruction(&I::GlobalSet(3));
        f.instruction(&I::I32Const(0));
        f.instruction(&I::End);
        defs.push(Def { name: "pr_init".into(), ty: ty_i32_ret, body: Body::Encoded(f) });
    }
    let run_prog_idx = n_imports + defs.len() as u32;
    {
        let mut f = Function::new([(1, ValType::I32)]);
        f.instruction(&I::Call(init_idx));
        f.instruction(&I::Drop);
        f.instruction(&I::I32Const(region_base as i32));
        f.instruction(&I::GlobalSet(1));
        for op in &img.ops {
            match op {
                InitOp::Fill { off, len, byte } => {
                    f.instruction(&I::I32Const((region_base + off) as i32));
                    f.instruction(&I::I32Const(*byte as i32));
                    f.instruction(&I::I32Const(*len as i32));
                    f.instruction(&I::MemoryFill(0));
                }
                InitOp::Copy { off, blob_off, len } => {
                    f.instruction(&I::I32Const((region_base + off) as i32));
                    f.instruction(&I::I32Const((template_base + blob_off) as i32));
                    f.instruction(&I::I32Const(*len as i32));
                    f.instruction(&I::MemoryCopy { src_mem: 0, dst_mem: 0 });
                }
            }
        }
        f.instruction(&I::I32Const(0));
        f.instruction(&I::GlobalSet(g_pc));
        f.instruction(&I::Loop(BlockType::Empty));
        f.instruction(&I::Call(run_idx));
        f.instruction(&I::LocalTee(0));
        f.instruction(&I::I32Const(-2));
        f.instruction(&I::I32Eq);
        f.instruction(&I::If(BlockType::Empty));
        f.instruction(&I::I32Const(opts.budget));
        f.instruction(&I::GlobalSet(3));
        f.instruction(&I::Br(1));
        f.instruction(&I::End);
        f.instruction(&I::End);
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::End);
        defs.push(Def { name: "pr_run".into(), ty: ty_i32_ret, body: Body::Encoded(f) });
    }

    // ── emit sections ────────────────────────────────────────────────────
    let mut module = Module::new();

    let mut ts = TypeSection::new();
    for (p, r) in &types.list {
        ts.ty().function(p.iter().copied(), r.iter().copied());
    }
    module.section(&ts);

    let mut imports = ImportSection::new();
    for g in &glue_list {
        imports.import("pr", g, EntityType::Function(glue_types(g).0));
    }
    module.section(&imports);

    let mut fs = FunctionSection::new();
    for d in &defs {
        fs.function(d.ty);
    }
    module.section(&fs);

    let mut tables = TableSection::new();
    tables.table(TableType {
        element_type: RefType::FUNCREF,
        minimum: table_funcs.len() as u64,
        maximum: Some(table_funcs.len() as u64),
        table64: false,
        shared: false,
    });
    module.section(&tables);

    let mut mems = wasm_encoder::MemorySection::new();
    mems.memory(MemoryType {
        minimum: data_pages as u64,
        maximum: Some(data_pages as u64),
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    if opts.multi_memory {
        mems.memory(MemoryType {
            minimum: arena_pages as u64,
            maximum: Some(arena_pages as u64),
            memory64: false,
            shared: false,
            page_size_log2: None,
        });
    }
    module.section(&mems);

    let mut globals = GlobalSection::new();
    let gt = GlobalType { val_type: ValType::I32, mutable: true, shared: false };
    globals.global(gt, &ConstExpr::i32_const(blob.stack_top as i32));
    for _ in 1..=5 {
        globals.global(gt, &ConstExpr::i32_const(0));
    }
    module.section(&globals);

    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);
    exports.export("arena", ExportKind::Memory, if opts.multi_memory { 1 } else { 0 });
    exports.export("pr_abi", ExportKind::Func, e_abi);
    exports.export("pr_variant", ExportKind::Func, e_variant);
    exports.export("pr_arena_base", ExportKind::Func, e_arena);
    exports.export("pr_init", ExportKind::Func, init_idx);
    exports.export("pr_run", ExportKind::Func, run_prog_idx);
    module.section(&exports);

    let mut elems = ElementSection::new();
    elems.active(Some(0), &ConstExpr::i32_const(0), Elements::Functions(std::borrow::Cow::Owned(table_funcs.clone())));
    module.section(&elems);

    // Code, in function-index order.
    let mut code = CodeSection::new();
    let mut mapper_types = blob_types.clone();
    let mut rt_bytes = 0usize;
    {
        let mut mapper = Mapper { func: &rt_final, types: &mapper_types, stack_global: 0 };
        for d in &defs {
            match &d.body {
                Body::Encoded(f) => {
                    code.function(f);
                }
                Body::Rt(bi) => {
                    let before = code.byte_len();
                    blob.emit_body(*bi, &mut mapper, &mut code).map_err(|e| GenError::new(0, e.to_string()))?;
                    rt_bytes += code.byte_len() - before;
                }
            }
        }
    }
    mapper_types.clear();
    module.section(&code);

    let mut data = DataSection::new();
    for (addr, bytes) in &blob.data {
        data.active(0, &ConstExpr::i32_const(*addr as i32), bytes.iter().copied());
    }
    if !pool.bytes.is_empty() {
        data.active(0, &ConstExpr::i32_const(consts_base as i32), pool.bytes.iter().copied());
    }
    if !img.blob.is_empty() {
        data.active(0, &ConstExpr::i32_const(template_base as i32), img.blob.iter().copied());
    }
    module.section(&data);

    // Names.
    let mut names = NameSection::new();
    let mut fnames = NameMap::new();
    for g in &glue_list {
        fnames.append(imp_of(g), &format!("pr.{g}"));
    }
    for (i, d) in defs.iter().enumerate() {
        fnames.append(n_imports + i as u32, &d.name);
    }
    names.functions(&fnames);
    module.section(&names);

    let manifest = serde_json::json!({
        "abi": ABI_VERSION,
        "generator": format!("cobolt-wasm {}", env!("CARGO_PKG_VERSION")),
        "project": opts.name,
        "variant": { "multi_memory": opts.multi_memory, "conformance": opts.conformance },
        "limits": {
            "arena_size": arena_total, "batch_size": opts.arena.batch, "answer_size": opts.arena.answer,
            "frame_size": opts.arena.frame, "budget_blocks": opts.budget, "data_pages": data_pages
        },
        "imports": glue_list,
        "regions": { "rt_workspace": rt_ws, "perform_stack": pstack_base, "constants": consts_base, "template": template_base, "region": region_base },
        "forms": [],
    });
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    module.section(&CustomSection { name: std::borrow::Cow::Borrowed("prmanifest"), data: std::borrow::Cow::Borrowed(&manifest_bytes) });

    let wasm = module.finish();
    let _ = BTreeMap::<u8, u8>::new();
    let sizes = Sizes {
        module_bytes: wasm.len(),
        rt_functions: closure.funcs.len(),
        rt_bytes,
        blocks: order.len(),
        data_pages,
    };
    Ok(Compiled { wasm, manifest, sizes })
}
