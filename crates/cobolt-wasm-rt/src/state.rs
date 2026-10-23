// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The routines' own state: a small register stack for numbers, a few flags,
//! and the address of the **workspace** the generator reserved for them.
//!
//! State lives in this crate's statics, which on wasm32 are bytes of the
//! module's data segment. Nothing here has to survive a suspension: a task is
//! suspended only *between* statements, never inside a numeric expression, so
//! the register stack is always empty at a suspension point.
//!
//! Buffers that need an **address** the host or the generator can name — the
//! command batch, the answer area — live in the workspace instead, because a
//! Rust `static` has an address only on wasm32 (the native tests have a
//! [`crate::mem::TestMem`] for that).

use crate::val::Val;

/// Depth of the numeric register stack. An expression nests this deep at most;
/// the generator refuses to emit a deeper one.
pub const NREG: usize = 24;
/// Temporaries a statement may save a computed subscript or bound in.
pub const NTEMP: usize = 8;

/// Workspace layout, relative to the address given to [`rt_init`](crate::rt_init).
pub const WS_BATCH: u32 = 0;
/// Bytes of command batch staged before a flush is forced.
pub const BATCH_SIZE: u32 = 16 * 1024;
pub const WS_ANSWER: u32 = WS_BATCH + BATCH_SIZE;
/// Bytes of query answer.
pub const ANSWER_SIZE: u32 = 16 * 1024;
/// Scratch for sender snapshots, string building and number formatting.
pub const WS_SCRATCH: u32 = WS_ANSWER + ANSWER_SIZE;
pub const SCRATCH_SIZE: u32 = 32 * 1024;
/// Character temporaries: where a character-valued intrinsic function leaves
/// its result for the operand (`T_CTEMP`) that names the slot.
pub const NCTEMP: usize = 4;
pub const CTEMP_SIZE: u32 = 4096;
pub const WS_CTEMP: u32 = WS_SCRATCH + SCRATCH_SIZE;
/// The path of the host command being built (ABI v1 §5).
pub const WS_PATH: u32 = WS_CTEMP + NCTEMP as u32 * CTEMP_SIZE;
pub const PATH_SIZE: u32 = 2048;
/// The group ⇄ JSON mapping's work area (see [`crate::json`]): the shadow copy
/// a decode writes before it commits, the path of the item being mapped (the
/// one an error names), and the paths of the variable tables a decode read.
pub const WS_JSON_SHADOW: u32 = WS_PATH + PATH_SIZE;
/// The largest group a decode can take, in bytes.
pub const JSON_SHADOW_SIZE: u32 = 32 * 1024;
pub const WS_JSON_PATH: u32 = WS_JSON_SHADOW + JSON_SHADOW_SIZE;
pub const JSON_PATH_SIZE: u32 = 1024;
pub const WS_JSON_ODO: u32 = WS_JSON_PATH + JSON_PATH_SIZE;
pub const JSON_ODO_SIZE: u32 = 4096;
/// A remote call's frames in memory 0 (WASM-010 T16): the request is built
/// here before the glue copies it to frame-out, and `pr_resume` copies the
/// answer here from frame-in, with its length in the word at `WS_FRAME_IN_LEN`
/// (`u32::MAX` when it did not fit).
pub const WS_FRAME_IN_LEN: u32 = WS_JSON_ODO + JSON_ODO_SIZE;
pub const WS_FRAME: u32 = WS_FRAME_IN_LEN + 8;
pub const FRAME_SIZE: u32 = 64 * 1024;
/// The line a `DISPLAY` statement is building: it goes to the host as one
/// command when the statement ends (`crate::disp`).
pub const WS_DISP: u32 = WS_FRAME + FRAME_SIZE;
pub const DISP_SIZE: u32 = 4096;
/// Total workspace size the generator must reserve.
pub const WS_SIZE: u32 = WS_DISP + DISP_SIZE;

/// Run-time error codes carried by the `ERROR` command (ABI v1 §5). The host
/// owns the message text, so the IDE and the page can translate it.
pub mod err {
    pub const SUBSCRIPT: u32 = 100;
    pub const REFMOD: u32 = 101;
    pub const DIVISION_BY_ZERO: u32 = 102;
    pub const REGISTER_OVERFLOW: u32 = 103;
    pub const BAD_OPERAND: u32 = 104;
    pub const SCRATCH_OVERFLOW: u32 = 105;
    /// More than 512 nested out-of-line PERFORMs.
    pub const PERFORM_DEPTH: u32 = 106;
    /// A LINKAGE item was used that the CALL did not pass.
    pub const UNBOUND_LINKAGE: u32 = 107;
    /// `CALL` of a program that is not in the module, with no `ON EXCEPTION`.
    pub const PROGRAM_NOT_FOUND: u32 = 108;
    /// The file heap cannot hold a `RELEASE`d record (a `SORT` has no FILE
    /// STATUS to report it in).
    pub const MEMORY_FULL: u32 = 109;
    /// `CALL … WITH WEBSOCKET LINKAGE` failed and the statement has no
    /// `ON EXCEPTION` (WASM-005 R9: a failed call is a run-time error).
    pub const REMOTE_FAILED: u32 = 110;
}

pub struct State {
    pub regs: [Val; NREG],
    pub sp: usize,
    pub temps: [Val; NTEMP],
    /// Set by a store that overflowed its receiver, or by a division by zero.
    pub size_err: bool,
    /// Set by a division by zero inside the current statement: the receivers
    /// are left alone.
    pub div0: bool,
    /// Address of the workspace.
    pub ws: u32,
    /// The form instance whose events the module is running.
    pub inst: u32,
    /// Bytes currently staged in the batch.
    pub batch_len: u32,
    /// Address of the program collating sequence (`[low][high][u16 × 256]`), or 0.
    pub collate: u32,
    /// The resolved sender of a `MOVE` in progress (several receivers).
    pub snd: Option<crate::mv::Sender>,
    /// Lengths of the character temporaries' contents.
    pub ctemp_len: [u32; NCTEMP],
    /// The state of `FUNCTION RANDOM`'s generator.
    pub random: u64,
    /// The host command path being built: bytes so far, and where its segment
    /// count and the current segment's argument count sit.
    pub path_len: u32,
    pub path_nseg_at: u32,
    pub path_nargs_at: u32,
    /// Character temporaries that hold a property read, whose text is a number
    /// when it spells one (bit `k`).
    pub ctemp_dynamic: u32,
    /// The argument count the next intrinsic-function call uses instead of its
    /// record's (an `ALL` over an `OCCURS … DEPENDING ON` table); `u32::MAX`
    /// when the record's stands.
    pub func_argc: u32,
    /// The file heap of in-memory files and `SORT` (see [`crate::idx`]):
    /// `[heap_base, heap_end)`, and where the next allocation looks first.
    pub heap_base: u32,
    pub heap_end: u32,
    pub heap_rover: u32,
    /// Bytes of the path at `WS_JSON_PATH` (the item the mapping is at, or the
    /// one its last error names).
    pub json_path_len: u32,
    /// Bytes of the `DISPLAY` line at `WS_DISP`.
    pub disp_len: u32,
}

impl State {
    pub const fn new() -> State {
        State { regs: [Val::ZERO; NREG], sp: 0, temps: [Val::ZERO; NTEMP], size_err: false, div0: false, ws: 0, inst: 0, batch_len: 0, collate: 0, snd: None, ctemp_len: [0; NCTEMP], random: crate::func::RANDOM_INITIAL, path_len: 0, path_nseg_at: 0, path_nargs_at: 0, ctemp_dynamic: 0, func_argc: u32::MAX, heap_base: 0, heap_end: 0, heap_rover: 0, json_path_len: 0, disp_len: 0 }
    }
}

impl Default for State {
    fn default() -> Self {
        State::new()
    }
}

#[cfg(target_arch = "wasm32")]
static mut STATE: State = State::new();

/// The state. The module is single-threaded, so one `&mut` at a time is a
/// property of the program, not something the type system can see.
#[cfg(target_arch = "wasm32")]
#[inline(always)]
pub fn st() -> &'static mut State {
    // SAFETY: wasm32 without atomics runs one thread; no routine re-enters
    // another while holding the reference across the call.
    unsafe { &mut *core::ptr::addr_of_mut!(STATE) }
}

#[cfg(not(target_arch = "wasm32"))]
std::thread_local! {
    static STATE: core::cell::Cell<*mut State> = const { core::cell::Cell::new(core::ptr::null_mut()) };
}

#[cfg(not(target_arch = "wasm32"))]
pub fn st() -> &'static mut State {
    STATE.with(|c| {
        if c.get().is_null() {
            c.set(std::boxed::Box::leak(std::boxed::Box::new(State::new())));
        }
        // SAFETY: thread-local, leaked box: unique per thread.
        unsafe { &mut *c.get() }
    })
}

/// Reset the state of this thread (native tests).
#[cfg(not(target_arch = "wasm32"))]
pub fn reset() {
    *st() = State::new();
}

/// Push a number on the register stack.
#[inline]
pub fn push(v: Val) {
    let s = st();
    if s.sp >= NREG {
        crate::fail(err::REGISTER_OVERFLOW);
    }
    s.regs[s.sp] = v;
    s.sp += 1;
}

/// Pop the top of the register stack.
#[inline]
pub fn pop() -> Val {
    let s = st();
    if s.sp == 0 {
        crate::fail(err::REGISTER_OVERFLOW);
    }
    s.sp -= 1;
    s.regs[s.sp]
}
