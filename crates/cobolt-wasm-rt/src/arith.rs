// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The register machine behind `ADD`, `SUBTRACT`, `MULTIPLY`, `DIVIDE` and
//! `COMPUTE`.
//!
//! The generator turns an arithmetic statement into a straight sequence of
//! calls: push operands, apply operators, store. Every statement shape of the
//! interpreter (`exec_add` … `exec_compute`) is a composition of these, with
//! the same intermediate values, so the results are the interpreter's.

use crate::codec;
use crate::opd::*;
use crate::state::{err, pop, push, st};
use crate::val::Val;

pub const OP_ADD: u32 = 0;
pub const OP_SUB: u32 = 1;
pub const OP_MUL: u32 = 2;
pub const OP_DIV: u32 = 3;
pub const OP_POW: u32 = 4;
pub const OP_NEG: u32 = 5;

pub const ST_ROUNDED: u32 = 1;
pub const ST_PHRASE: u32 = 2;

pub fn push_operand(base: u32, o: u32) {
    let op = parse(o);
    push(value(base, &op));
}

/// Apply a binary operator to the two topmost registers (`a` below `b`).
pub fn apply(op: u32) {
    if op == OP_NEG {
        let a = pop();
        push(a.neg());
        return;
    }
    let b = pop();
    let a = pop();
    let r = match op {
        OP_ADD => a.add(&b),
        OP_SUB => a.sub(&b),
        OP_MUL => a.mul(&b),
        OP_DIV => match a.div(&b) {
            Some(q) => q,
            None => {
                // Division by zero is a size error (COBOL-85 6.4.4): the
                // statement's receivers are not stored.
                st().size_err = true;
                st().div0 = true;
                Val::ZERO
            }
        },
        OP_POW => a.pow(&b),
        _ => crate::fail(err::BAD_OPERAND),
    };
    push(r);
}

/// Copy the register `depth` places below the top onto the top.
pub fn pick(depth: u32) {
    let s = st();
    let n = s.sp;
    if (depth as usize) >= n {
        crate::fail(err::REGISTER_OVERFLOW);
    }
    let v = s.regs[n - 1 - depth as usize];
    push(v);
}

/// Pop a register and store it into the item `o`; returns `true` on a size
/// error. `flags`: [`ST_ROUNDED`], [`ST_PHRASE`].
pub fn store(base: u32, o: u32, flags: u32) -> bool {
    let v = pop();
    let s = st();
    if s.div0 {
        // A division by zero already counted as the size error; the receiver
        // keeps its value.
        return true;
    }
    let op = parse(o);
    let loc = locate(base, &op);
    let over = codec::store_value(&loc, v, flags & ST_ROUNDED != 0, flags & ST_PHRASE != 0);
    if over {
        st().size_err = true;
    }
    over
}

/// Truncate the top register to the scale of item `o` (for `REMAINDER`).
pub fn truncate_to_item(base: u32, o: u32) {
    let op = parse(o);
    let loc = locate(base, &op);
    let v = pop();
    let t = match v {
        Val::Dec(n) if loc.fd.scale >= 0 => Val::Dec(n.truncate_to(loc.fd.scale as u8)),
        other => other,
    };
    push(t);
}

pub fn begin_statement() {
    let s = st();
    s.size_err = false;
    s.div0 = false;
    s.sp = 0;
}
