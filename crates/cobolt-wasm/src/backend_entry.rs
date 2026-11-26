// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The entry of a Backend module (ABI A59, spec 114): `pr_backend_call(export) -> i32`.
//!
//! A Backend module holds every program a Backend exports, each in a region of its own with its own
//! initial image (the way a project's forms are held). The entry takes the **request frame** the host
//! wrote in memory 0, and for the export it is asked to run:
//!
//! 1. runs the module's start-up (`pr_init`), points the program's region at its image, and its perform
//!    stack at its own;
//! 2. points each `LINKAGE` record's slot at storage of its own (the way a `CALL … USING BY REFERENCE`
//!    does) and puts the record in its initial state with the `INITIALIZE` the lowering made;
//! 3. reads each text of the frame into its record with the module's group ⇄ JSON routines — all or
//!    nothing — and answers `REFUSED` with the rt's error code and the member's path when one is refused;
//! 4. runs the program until it ends (the block loop `pr_run` has, which never yields: time and cancel
//!    are the host's);
//! 5. writes every parameter and the `RETURNING` record back as JSON, into the **response frame**.
//!
//! The frames are `cobolt_wasm_abi::backend`'s. Everything here is straight-line wasm over constants the
//! lowering fixed (addresses, descriptors, offsets), so the entry has no data of its own.

use crate::lower::{BackendLowered, BackendRecord};
use cobolt_wasm_abi::backend::result;
use wasm_encoder::{BlockType, Function, Instruction as I, MemArg, ValType};

/// One exported program, as the entry reaches it.
pub(crate) struct EntryProgram<'a> {
    /// Where its region starts in memory 0.
    pub region_start: u32,
    /// Where its perform stack starts, from the region base.
    pub stack_off: u32,
    /// The program counter of its first block.
    pub entry_pc: u32,
    /// The function that copies its initial image over the region (takes the region base).
    pub init_fn: u32,
    pub records: &'a BackendLowered,
}

/// What the entry needs of the module around it.
pub(crate) struct EntryCtx<'a> {
    /// The function index of a linked routine.
    pub rt: &'a dyn Fn(&str) -> u32,
    /// `pr_init`.
    pub pr_init: u32,
    /// The block dispatcher: runs from the program counter until the task ends (`-1`), stops (`-4`) or
    /// runs out of budget (`-2`).
    pub run: u32,
    /// The global that holds the program counter.
    pub g_pc: u32,
    pub budget: i32,
    pub request_at: u32,
    pub request_cap: u32,
    pub response_at: u32,
    pub response_cap: u32,
}

const BASE_LOCAL: u32 = 1;
const CURSOR: u32 = 2;
const COUNT: u32 = 3;
const LEN: u32 = 4;
const RC: u32 = 5;

/// The rt's code for an answer that does not fit (`cobolt_wasm_rt::json::E_NO_ROOM`).
const E_NO_ROOM: i32 = cobolt_wasm_rt::json::E_NO_ROOM;
/// The longest path a refusal carries (`cobolt_wasm_rt::state::JSON_PATH_SIZE`).
const PATH_CAP: u32 = cobolt_wasm_rt::state::JSON_PATH_SIZE;

fn c(f: &mut Function, v: u32) {
    f.instruction(&I::I32Const(v as i32));
}

fn ci(f: &mut Function, v: i32) {
    f.instruction(&I::I32Const(v));
}

fn load(f: &mut Function) {
    f.instruction(&I::I32Load(MemArg { offset: 0, align: 0, memory_index: 0 }));
}

fn store(f: &mut Function) {
    f.instruction(&I::I32Store(MemArg { offset: 0, align: 0, memory_index: 0 }));
}

fn ret_const(f: &mut Function, v: i32) {
    ci(f, v);
    f.instruction(&I::Return);
}

/// `if <cond on the stack> { return v }`.
fn return_if(f: &mut Function, v: i32) {
    f.instruction(&I::If(BlockType::Empty));
    ret_const(f, v);
    f.instruction(&I::End);
}

/// The distinct records of a program, in the order the entry binds them: the parameters, then the
/// `RETURNING` record when it is not one of them.
fn distinct(r: &BackendLowered) -> Vec<&BackendRecord> {
    let mut v: Vec<&BackendRecord> = r.params.iter().collect();
    if let Some(ret) = &r.returning {
        if !r.params.iter().any(|p| p.store == ret.store) {
            v.push(ret);
        }
    }
    v
}

/// The body of `pr_backend_call`.
pub(crate) fn emit(ctx: &EntryCtx<'_>, programs: &[EntryProgram<'_>]) -> Function {
    // locals: 0 export (parameter) · 1 region base · 2 cursor · 3 count · 4 length · 5 result of a call
    let mut f = Function::new([(5, ValType::I32)]);
    f.instruction(&I::LocalGet(0));
    c(&mut f, programs.len() as u32);
    f.instruction(&I::I32GeU);
    return_if(&mut f, result::BAD_REQUEST);
    f.instruction(&I::Call(ctx.pr_init));
    f.instruction(&I::Drop);
    for (k, p) in programs.iter().enumerate() {
        f.instruction(&I::LocalGet(0));
        c(&mut f, k as u32);
        f.instruction(&I::I32Eq);
        f.instruction(&I::If(BlockType::Empty));
        program(&mut f, ctx, p);
        f.instruction(&I::End);
    }
    ci(&mut f, result::BAD_REQUEST);
    f.instruction(&I::End);
    f
}

fn program(f: &mut Function, ctx: &EntryCtx<'_>, p: &EntryProgram<'_>) {
    let req_end = ctx.request_at + ctx.request_cap;
    let resp_end = ctx.response_at + ctx.response_cap;
    let records = distinct(p.records);
    let n_params = p.records.params.len() as u32;

    // The region: its base, its image, its perform stack.
    c(f, p.region_start);
    f.instruction(&I::LocalSet(BASE_LOCAL));
    c(f, p.region_start);
    f.instruction(&I::GlobalSet(1));
    f.instruction(&I::LocalGet(BASE_LOCAL));
    f.instruction(&I::Call(p.init_fn));
    c(f, p.region_start + p.stack_off);
    f.instruction(&I::GlobalSet(2));

    // The request frame's count.
    c(f, ctx.request_at);
    load(f);
    f.instruction(&I::LocalSet(COUNT));
    f.instruction(&I::LocalGet(COUNT));
    c(f, n_params);
    f.instruction(&I::I32GtU);
    return_if(f, result::BAD_REQUEST);

    // Every record: its slot points at its storage, and it starts as INITIALIZE leaves it.
    for r in &records {
        f.instruction(&I::LocalGet(BASE_LOCAL));
        c(f, r.slot);
        f.instruction(&I::I32Add);
        f.instruction(&I::LocalGet(BASE_LOCAL));
        c(f, r.store);
        f.instruction(&I::I32Add);
        store(f);
        f.instruction(&I::LocalGet(BASE_LOCAL));
        c(f, r.init);
        f.instruction(&I::Call((ctx.rt)("rt_initialize")));
    }

    // Each text the host sent, into its record.
    c(f, ctx.request_at + 4);
    f.instruction(&I::LocalSet(CURSOR));
    for (i, r) in p.records.params.iter().enumerate() {
        f.instruction(&I::LocalGet(COUNT));
        c(f, i as u32);
        f.instruction(&I::I32GtU);
        f.instruction(&I::If(BlockType::Empty));
        // The length word and the text it announces lie inside the request area.
        f.instruction(&I::LocalGet(CURSOR));
        c(f, 4);
        f.instruction(&I::I32Add);
        c(f, req_end);
        f.instruction(&I::I32GtU);
        return_if(f, result::BAD_REQUEST);
        f.instruction(&I::LocalGet(CURSOR));
        load(f);
        f.instruction(&I::LocalSet(LEN));
        f.instruction(&I::LocalGet(LEN));
        c(f, req_end);
        f.instruction(&I::LocalGet(CURSOR));
        f.instruction(&I::I32Sub);
        c(f, 4);
        f.instruction(&I::I32Sub);
        f.instruction(&I::I32GtU);
        return_if(f, result::BAD_REQUEST);
        // rt_json_decode(desc, addr, text, len) -> 0 or a negated code
        c(f, r.desc);
        f.instruction(&I::LocalGet(BASE_LOCAL));
        c(f, r.store);
        f.instruction(&I::I32Add);
        f.instruction(&I::LocalGet(CURSOR));
        c(f, 4);
        f.instruction(&I::I32Add);
        f.instruction(&I::LocalGet(LEN));
        f.instruction(&I::Call((ctx.rt)("rt_json_decode")));
        f.instruction(&I::LocalSet(RC));
        f.instruction(&I::LocalGet(RC));
        f.instruction(&I::If(BlockType::Empty));
        refusal(f, ctx, result::REFUSED);
        f.instruction(&I::End);
        // the next text
        f.instruction(&I::LocalGet(CURSOR));
        c(f, 4);
        f.instruction(&I::I32Add);
        f.instruction(&I::LocalGet(LEN));
        f.instruction(&I::I32Add);
        f.instruction(&I::LocalSet(CURSOR));
        f.instruction(&I::End);
    }

    // Run to the end: the block loop `pr_run` has, entered at this program's first block.
    ci(f, ctx.budget);
    f.instruction(&I::GlobalSet(3));
    c(f, p.entry_pc);
    f.instruction(&I::GlobalSet(ctx.g_pc));
    f.instruction(&I::Loop(BlockType::Empty));
    f.instruction(&I::Call(ctx.run));
    ci(f, -2);
    f.instruction(&I::I32Eq);
    f.instruction(&I::If(BlockType::Empty));
    ci(f, ctx.budget);
    f.instruction(&I::GlobalSet(3));
    f.instruction(&I::Br(1));
    f.instruction(&I::End);
    f.instruction(&I::End);

    // The response frame: every parameter, then the RETURNING record.
    c(f, ctx.response_at);
    c(f, n_params);
    store(f);
    c(f, ctx.response_at + 4);
    f.instruction(&I::LocalSet(CURSOR));
    for r in &p.records.params {
        encode(f, ctx, r, resp_end);
    }
    // has_returning
    f.instruction(&I::LocalGet(CURSOR));
    c(f, 4);
    f.instruction(&I::I32Add);
    c(f, resp_end);
    f.instruction(&I::I32GtU);
    return_if(f, result::TOO_LARGE);
    f.instruction(&I::LocalGet(CURSOR));
    ci(f, p.records.returning.is_some() as i32);
    store(f);
    f.instruction(&I::LocalGet(CURSOR));
    c(f, 4);
    f.instruction(&I::I32Add);
    f.instruction(&I::LocalSet(CURSOR));
    if let Some(r) = &p.records.returning {
        encode(f, ctx, r, resp_end);
    }
    ret_const(f, result::ANSWERED);
}

/// The request was refused: the rt's code, the member's path, and the result. The path is what the rt
/// kept of the item the error names (`rt_json_error_path`), at most `PATH_CAP` bytes and what the response
/// area can hold after the two words.
fn refusal(f: &mut Function, ctx: &EntryCtx<'_>, status: i32) {
    c(f, ctx.response_at);
    ci(f, 0);
    f.instruction(&I::LocalGet(RC));
    f.instruction(&I::I32Sub);
    store(f);
    // rt_json_error_path(out, cap) -> the length written
    c(f, ctx.response_at + 8);
    c(f, PATH_CAP.min(ctx.response_cap.saturating_sub(8)));
    f.instruction(&I::Call((ctx.rt)("rt_json_error_path")));
    f.instruction(&I::LocalSet(LEN));
    c(f, ctx.response_at + 4);
    f.instruction(&I::LocalGet(LEN));
    store(f);
    ret_const(f, status);
}

/// Write `len` and the JSON of `r` at the cursor, and move the cursor past them; an answer that does not
/// fit is `TOO_LARGE`.
fn encode(f: &mut Function, ctx: &EntryCtx<'_>, r: &BackendRecord, resp_end: u32) {
    let _ = ctx;
    f.instruction(&I::LocalGet(CURSOR));
    c(f, 4);
    f.instruction(&I::I32Add);
    c(f, resp_end);
    f.instruction(&I::I32GtU);
    return_if(f, result::TOO_LARGE);
    // rt_json_encode(desc, addr, out, cap) -> length or a negated code
    c(f, r.desc);
    f.instruction(&I::LocalGet(BASE_LOCAL));
    c(f, r.store);
    f.instruction(&I::I32Add);
    f.instruction(&I::LocalGet(CURSOR));
    c(f, 4);
    f.instruction(&I::I32Add);
    c(f, resp_end);
    f.instruction(&I::LocalGet(CURSOR));
    f.instruction(&I::I32Sub);
    c(f, 4);
    f.instruction(&I::I32Sub);
    f.instruction(&I::Call((ctx.rt)("rt_json_encode")));
    f.instruction(&I::LocalSet(RC));
    f.instruction(&I::LocalGet(RC));
    ci(f, 0);
    f.instruction(&I::I32LtS);
    f.instruction(&I::If(BlockType::Empty));
    f.instruction(&I::LocalGet(RC));
    ci(f, -E_NO_ROOM);
    f.instruction(&I::I32Eq);
    return_if(f, result::TOO_LARGE);
    // Any other refusal to write what the program's own records hold is the program's data, not a defect of the
    // module (a count the program left out of its range, bytes that are not text): the interpreter answers it
    // with a validation error naming the member, and so does this, with the member's path.
    refusal(f, ctx, result::UNWRITABLE);
    f.instruction(&I::End);
    f.instruction(&I::LocalGet(CURSOR));
    f.instruction(&I::LocalGet(RC));
    store(f);
    f.instruction(&I::LocalGet(CURSOR));
    c(f, 4);
    f.instruction(&I::I32Add);
    f.instruction(&I::LocalGet(RC));
    f.instruction(&I::I32Add);
    f.instruction(&I::LocalSet(CURSOR));
}
