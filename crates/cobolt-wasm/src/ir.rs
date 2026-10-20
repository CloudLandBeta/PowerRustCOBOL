// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The generator's intermediate form: blocks of instructions with symbolic
//! references, resolved to indices when the module is assembled.

use wasm_encoder::Instruction;

/// A basic block: becomes one WebAssembly function `() -> i32` returning the
/// id (table index) of the block to run next, or a negative reserved code.
pub type BlockId = usize;

/// Reserved return codes of a block function.
pub mod ret {
    /// The task finished.
    pub const DONE: i32 = -1;
    /// `STOP RUN`: the application ends.
    pub const STOP: i32 = -4;
    /// `WAIT-EVENT`: the form waits for the host's next event; `Resume` holds
    /// the block to continue in.
    pub const PARK: i32 = -5;
    /// Waiting for a remote answer; `Resume` holds the block to continue in.
    pub const SUSPEND: i32 = -6;
}

/// The module's own mutable globals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum G {
    /// Base address of the current form instance's region.
    Base,
    /// Perform-stack pointer.
    Psp,
    /// Remaining block budget before the module yields.
    Fuel,
    /// The id of the statement about to run (for run-time error positions).
    Line,
    /// Where a parked or suspended task continues.
    Resume,
}

/// Functions the module itself defines besides blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Glue {
    /// `glue.flush(addr, len)` — hand the staged batch to the host.
    Flush,
    /// `glue.abort(code)` — report a run-time error and trap.
    Abort,
}

#[derive(Debug, Clone)]
pub enum Ins {
    Raw(Instruction<'static>),
    /// Call a routine of the linked blob by name.
    CallRt(&'static str),
    CallGlue(Glue),
    /// `i32.const` of a block's table index.
    Pc(BlockId),
    GlobalGet(G),
    GlobalSet(G),
    /// Call the function that puts program unit `n`'s storage back to its
    /// initial state (`CANCEL`).
    InitUnit(usize),
}

impl From<Instruction<'static>> for Ins {
    fn from(i: Instruction<'static>) -> Ins {
        Ins::Raw(i)
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    pub name: String,
    pub code: Vec<Ins>,
    pub succ: Vec<BlockId>,
    /// True once the block has been terminated.
    pub done: bool,
}
