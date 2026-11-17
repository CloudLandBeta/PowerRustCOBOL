// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `EXEC SQL` — **a desktop module only** (ABI A41, DESKTOP-WASM-001 T41).
//!
//! The runner runs the statement with the interpreter's own SQL code; these routines carry the
//! program's half. They send the value of each cell the statement may read or write (each
//! elementary item a host variable, an indicator or a status item stands for), call the runner's
//! import `sql_exec`, and store the cells the statement changed. A value crosses as the interpreter
//! holds it: a number as its decimal mantissa and scale, a floating item as a float, anything else
//! as its bytes (`cobolt_wasm_abi::sql::wire`).
//!
//! A statement is described by a static spec record of words:
//!
//! ```text
//!  0 stmt   the statement's number in the module's `prsql` section
//!  4 n      the number of cells
//!  8 cells  n operand addresses, in the section's order
//! ```
//!
//! The values go out in the scratch area and the answer comes back over them; the generator sends a
//! statement the scratch area cannot hold down the Cargo path.

use crate::codec;
use crate::fd::{K_ALNUM, K_ALNUM_EDITED, K_FLOAT32, K_FLOAT64, K_NUM_EDITED};
use crate::host;
use crate::mem;
use crate::opd::{locate, parse, Loc};
use crate::state::{err, st, SCRATCH_SIZE, WS_SCRATCH};
use crate::val::Val;
use cobolt_decimal::CobolNumeric;

/// `cobolt_wasm_abi::sql::wire`, which this crate cannot link (`cobolt-wasm`'s tests hold the two equal).
pub mod wire {
    pub const DEC: u8 = 1;
    pub const FLOAT: u8 = 2;
    pub const BYTES: u8 = 3;
    pub const OUT_SUCCESS: u8 = 0;
    pub const OUT_WARNING: u8 = 1;
    pub const OUT_NOT_FOUND: u8 = 2;
    pub const OUT_ERROR: u8 = 3;
}

/// The most bytes one cell can take in either direction: its tag and its value, a number's 17
/// bytes, or a count and the item's bytes. The answer adds the cell's number.
pub fn cell_bound(len: u32) -> u32 {
    1 + 17u32.max(4 + len)
}

fn rd(addr: u32) -> u32 {
    mem::rd_u32(addr)
}

/// Cell `k` of statement `spec`. Its subscripts are constants, so it is the same item each time.
fn cell(base: u32, spec: u32, k: u32) -> Loc {
    locate(base, &parse(rd(spec + 8 + 4 * k)))
}

/// A number in an alphanumeric item: its digits, without sign or point, left-justified and padded with
/// spaces, as the interpreter shows a number it holds in such an item (a status item declared `PIC X`).
fn put_digits(loc: &Loc, n: &CobolNumeric) {
    let mut digits = [0u8; 40];
    let mut m = n.mantissa.unsigned_abs();
    let mut i = digits.len();
    loop {
        i -= 1;
        digits[i] = b'0' + (m % 10) as u8;
        m /= 10;
        if m == 0 || i == 0 {
            break;
        }
    }
    let src = &digits[i..];
    let keep = (src.len() as u32).min(loc.len);
    mem::bytes_mut(loc.addr, keep).copy_from_slice(&src[..keep as usize]);
    mem::fill(loc.addr + keep, loc.len - keep, b' ');
}

/// Whether an item's value is its bytes, as the interpreter holds an alphanumeric or edited item.
fn is_text(kind: u8) -> bool {
    matches!(kind, K_ALNUM | K_ALNUM_EDITED | K_NUM_EDITED)
}

/// Write one cell's value at `at`; returns the bytes written.
fn put_cell(loc: &Loc, at: u32) -> u32 {
    if is_text(loc.fd.kind) {
        mem::wr_u8(at, wire::BYTES);
        mem::wr_u32(at + 1, loc.len);
        mem::copy(loc.addr, at + 5, loc.len);
        return 5 + loc.len;
    }
    if matches!(loc.fd.kind, K_FLOAT32 | K_FLOAT64) {
        let f = codec::load_value(loc).as_f64();
        mem::wr_u8(at, wire::FLOAT);
        mem::bytes_mut(at + 1, 8).copy_from_slice(&f.to_le_bytes());
        return 9;
    }
    match codec::load_value(loc) {
        Val::Dec(n) => {
            mem::wr_u8(at, wire::DEC);
            mem::bytes_mut(at + 1, 16).copy_from_slice(&n.mantissa.to_le_bytes());
            mem::wr_u8(at + 17, n.decimals);
            18
        }
        Val::F(f) => {
            mem::wr_u8(at, wire::FLOAT);
            mem::bytes_mut(at + 1, 8).copy_from_slice(&f.to_le_bytes());
            9
        }
    }
}

/// Store the value at `at` into the cell; returns the bytes it took.
fn take_cell(loc: &Loc, at: u32) -> u32 {
    match mem::rd_u8(at) {
        wire::DEC => {
            let mantissa = mem::rd_i128(at + 1);
            let n = CobolNumeric::new(mantissa, mem::rd_u8(at + 17));
            if loc.fd.kind == K_ALNUM || loc.fd.kind == K_ALNUM_EDITED {
                put_digits(loc, &n);
            } else {
                codec::store_value(loc, Val::Dec(n), false, false);
            }
            18
        }
        wire::FLOAT => {
            let mut b = [0u8; 8];
            b.copy_from_slice(mem::bytes(at + 1, 8));
            codec::store_value(loc, Val::F(f64::from_le_bytes(b)), false, false);
            9
        }
        _ => {
            let n = rd(at + 1);
            let bytes = mem::bytes(at + 5, n);
            if is_text(loc.fd.kind) {
                // The item's bytes, padded with spaces or cut to its length.
                let keep = n.min(loc.len);
                mem::copy(at + 5, loc.addr, keep);
                mem::fill(loc.addr + keep, loc.len - keep, b' ');
            } else {
                let v = codec::parse_text(bytes).map(Val::Dec).unwrap_or(Val::ZERO);
                codec::store_value(loc, v, false, false);
            }
            5 + n
        }
    }
}

/// Run `EXEC SQL` statement `spec` (see the module's header). Returns the condition it ended in
/// (`wire::OUT_*`), for the generated code's `WHENEVER`.
pub fn exec(base: u32, spec: u32) -> u32 {
    let stmt = rd(spec);
    let n = rd(spec + 4);
    let buf = st().ws + WS_SCRATCH;
    let mut at = buf;
    for k in 0..n {
        at += put_cell(&cell(base, spec, k), at);
    }
    // SAFETY: the runner reads the values and writes the answer within `SCRATCH_SIZE` bytes at `buf`.
    let got = unsafe { host::sql_exec(stmt, buf, at - buf, SCRATCH_SIZE) };
    if got < 5 {
        crate::fail_text(err::BAD_OPERAND, &[crate::Part::T(b"the runner could not run EXEC SQL statement "), crate::Part::N(stmt)]);
    }
    let outcome = mem::rd_u8(buf) as u32;
    let count = rd(buf + 1);
    let mut at = buf + 5;
    for _ in 0..count {
        let k = rd(at);
        at += 4;
        match k < n {
            true => at += take_cell(&cell(base, spec, k), at),
            false => crate::fail_text(err::BAD_OPERAND, &[crate::Part::T(b"the runner answered a cell EXEC SQL statement "), crate::Part::N(stmt), crate::Part::T(b" does not have")]),
        }
    }
    outcome
}

/// `EXEC SQL`: see [`exec`].
#[no_mangle]
pub extern "C" fn rt_sql_exec(base: u32, spec: u32) -> u32 {
    exec(base, spec)
}
