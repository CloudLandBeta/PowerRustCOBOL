// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Operand records: how the generated code names "this data item, or that
//! literal" to a routine.
//!
//! The generator emits one record per reference in the source, as static data.
//! A routine is handed the **record's address** and the **base address of the
//! current form instance's region**, and does the rest — adding the offset,
//! evaluating subscripts and reference modification, reading the descriptor.
//! That keeps the generated code short and puts every addressing rule in one
//! place that has unit tests.
//!
//! ```text
//!  0  tag      u8   T_ITEM | T_LIT_NUM | T_LIT_ALNUM | T_FIG | T_ALL
//!  1  nsub     u8   number of subscripts (0..=7)
//!  2  flags    u8   OF_REFMOD
//!  3  fig      u8   FIG_* for T_FIG
//!  4  fd       12   the field descriptor (see fd.rs)
//! 16  off      u32  T_ITEM: offset from the region base
//!                   T_LIT_NUM: address of 17 bytes (i128 LE mantissa, u8 scale)
//!                   T_LIT_ALNUM / T_ALL: address of the bytes
//! 20  len      u32  T_LIT_ALNUM / T_ALL: byte count
//! 24  subs     nsub × 20:  opd u32, stride u32, max u32, odo u32, add i32
//!     refmod   if OF_REFMOD: start_opd u32, len_opd u32 (0 = to the end)
//! ```
//!
//! A subscript's `max` is the table's declared upper bound; `odo` is the
//! address of the operand record of the `DEPENDING ON` item, or 0; `add` is the
//! constant of a relative subscript (`I + 1`, `J - 2`).

use crate::codec;
use crate::fd::{Fd, FD_SIZE, K_ALNUM};
use crate::mem;
use crate::state::err;
use crate::val::Val;

pub const T_ITEM: u8 = 0;
pub const T_LIT_NUM: u8 = 1;
pub const T_LIT_ALNUM: u8 = 2;
pub const T_FIG: u8 = 3;
pub const T_ALL: u8 = 4;
/// A value saved in one of the statement's temporaries (`off` is its index).
pub const T_TEMP: u8 = 5;
/// The bytes a character-valued function left in character temporary `off`.
pub const T_CTEMP: u8 = 6;

pub const FIG_ZERO: u8 = 0;
pub const FIG_SPACE: u8 = 1;
pub const FIG_HIGH: u8 = 2;
pub const FIG_LOW: u8 = 3;
pub const FIG_QUOTE: u8 = 4;
pub const FIG_NULL: u8 = 5;

pub const OF_REFMOD: u8 = 1;
/// A LINKAGE item: `len` is the region offset of the slot holding the address
/// of the caller's record, and `off` is relative to that record.
pub const OF_LINK: u8 = 2;

pub const HDR: u32 = 24;
pub const SUB_SIZE: u32 = 20;

/// The parsed header of an operand record.
#[derive(Debug, Clone, Copy)]
pub struct Opd {
    pub addr: u32,
    pub tag: u8,
    pub nsub: u8,
    pub flags: u8,
    pub fig: u8,
    pub fd: Fd,
    pub off: u32,
    pub len: u32,
}

pub fn parse(addr: u32) -> Opd {
    let h = mem::bytes(addr, HDR);
    Opd {
        addr,
        tag: h[0],
        nsub: h[1],
        flags: h[2],
        fig: h[3],
        fd: Fd::parse(&h[4..4 + FD_SIZE]),
        off: u32::from_le_bytes([h[16], h[17], h[18], h[19]]),
        len: u32::from_le_bytes([h[20], h[21], h[22], h[23]]),
    }
}

/// Where an operand lives, once subscripts and reference modification have
/// been applied.
#[derive(Debug, Clone, Copy)]
pub struct Loc {
    pub addr: u32,
    pub len: u32,
    pub fd: Fd,
}

/// The fill byte of a figurative constant.
pub fn fig_byte(fig: u8) -> u8 {
    // Under a PROGRAM COLLATING SEQUENCE, LOW-VALUE and HIGH-VALUE name the
    // characters at the ends of that sequence.
    let c = crate::state::st().collate;
    if c != 0 && (fig == FIG_HIGH || fig == FIG_LOW) {
        let b = mem::bytes(c, 2);
        return if fig == FIG_LOW { b[0] } else { b[1] };
    }
    match fig {
        FIG_ZERO => b'0',
        FIG_SPACE => b' ',
        FIG_HIGH => 0xFF,
        FIG_LOW | FIG_NULL => 0x00,
        FIG_QUOTE => b'"',
        _ => b' ',
    }
}

/// The integer value of a subscript or reference-modification operand.
fn index_value(base: u32, opd_addr: u32) -> i64 {
    let o = parse(opd_addr);
    match value(base, &o) {
        Val::Dec(n) => (n.mantissa / cobolt_decimal::pow10(n.decimals as u32).unwrap_or(1)) as i64,
        Val::F(f) => f as i64,
    }
}

/// The location of a **data item or alphanumeric literal**.
///
/// Literals resolve to their bytes in the constant area; figuratives and `ALL`
/// have no single location (they fill a receiver) and resolve to a one-byte
/// location here only for the comparison code, which treats them separately.
pub fn locate(base: u32, o: &Opd) -> Loc {
    locate_sized(base, o, true)
}

/// As [`locate`], but a group holding a variable-length table is as long as the
/// table's declared maximum makes it: the receiver of a `MOVE` takes the whole
/// of what it is sent (COBOL-85 5.8.3, NC247A MOV-TEST-F1-6).
pub fn locate_full(base: u32, o: &Opd) -> Loc {
    locate_sized(base, o, false)
}

fn locate_sized(base: u32, o: &Opd, current: bool) -> Loc {
    match o.tag {
        T_ITEM => {
            let mut addr = if o.flags & OF_LINK != 0 {
                let p = mem::rd_u32(base.wrapping_add(o.len));
                if p == 0 {
                    crate::fail(err::UNBOUND_LINKAGE);
                }
                p.wrapping_add(o.off)
            } else {
                base.wrapping_add(o.off)
            };
            let mut sp = o.addr + HDR;
            for _ in 0..o.nsub {
                let s = mem::bytes(sp, SUB_SIZE);
                let sub_opd = u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
                let stride = u32::from_le_bytes([s[4], s[5], s[6], s[7]]);
                let max = u32::from_le_bytes([s[8], s[9], s[10], s[11]]);
                let odo = u32::from_le_bytes([s[12], s[13], s[14], s[15]]);
                let add = i32::from_le_bytes([s[16], s[17], s[18], s[19]]);
                let i = index_value(base, sub_opd) + add as i64;
                let mut bound = max as i64;
                if odo != 0 {
                    // The current count of a variable-length table. The
                    // physical bound still protects memory: a count above the
                    // declared maximum is clamped to it.
                    let c = index_value(base, odo);
                    if c >= 0 && c < bound {
                        bound = c;
                    }
                }
                if i < 1 || i > bound {
                    crate::fail(err::SUBSCRIPT);
                }
                addr = addr.wrapping_add(((i - 1) as u32).wrapping_mul(stride));
                sp += SUB_SIZE;
            }
            let mut loc = Loc { addr, len: o.fd.len, fd: o.fd };
            if current && o.fd.kind == crate::fd::K_GROUP && o.fd.aux != 0 {
                // A group holding a variable-length table is as long as the
                // table's current count makes it: `[stride, max, counter]`.
                let r = mem::bytes(o.fd.aux, 12);
                let stride = u32::from_le_bytes([r[0], r[1], r[2], r[3]]) as i64;
                let max = u32::from_le_bytes([r[4], r[5], r[6], r[7]]) as i64;
                let counter = u32::from_le_bytes([r[8], r[9], r[10], r[11]]);
                let c = index_value(base, counter).clamp(0, max);
                let len = (o.fd.len as i64 - (max - c) * stride).max(0) as u32;
                loc.len = len;
                loc.fd.len = len;
            }
            if o.flags & OF_REFMOD != 0 {
                let r = mem::bytes(sp, 8);
                let start_opd = u32::from_le_bytes([r[0], r[1], r[2], r[3]]);
                let len_opd = u32::from_le_bytes([r[4], r[5], r[6], r[7]]);
                let start = index_value(base, start_opd);
                if start < 1 || start as u64 > loc.len as u64 {
                    crate::fail(err::REFMOD);
                }
                let begin = (start - 1) as u32;
                let n = if len_opd == 0 {
                    loc.len - begin
                } else {
                    let l = index_value(base, len_opd);
                    if l < 1 || (begin as i64 + l) > loc.len as i64 {
                        crate::fail(err::REFMOD);
                    }
                    l as u32
                };
                // A reference-modified item is a bare character string,
                // whatever it was declared as.
                loc = Loc {
                    addr: loc.addr + begin,
                    len: n,
                    fd: Fd { kind: K_ALNUM, flags: 0, digits: 0, scale: 0, len: n, aux: 0 },
                };
            }
            loc
        }
        T_CTEMP => {
            let (addr, len) = crate::func::ctemp_loc(o.off);
            Loc { addr, len, fd: Fd { kind: K_ALNUM, flags: 0, digits: 0, scale: 0, len, aux: 0 } }
        }
        T_LIT_ALNUM | T_ALL => Loc {
            addr: o.off,
            len: o.len,
            fd: Fd { kind: K_ALNUM, flags: 0, digits: 0, scale: 0, len: o.len, aux: 0 },
        },
        T_FIG => Loc {
            addr: o.addr + 3, // the `fig` byte itself; callers read `fig_byte`
            len: 1,
            fd: Fd { kind: K_ALNUM, flags: 0, digits: 0, scale: 0, len: 1, aux: 0 },
        },
        _ => Loc {
            addr: o.off,
            len: 0,
            fd: Fd { kind: K_ALNUM, flags: 0, digits: 0, scale: 0, len: 0, aux: 0 },
        },
    }
}

/// The numeric value of an operand: a numeric literal, a numeric item, or —
/// as the interpreter does — an alphanumeric item read as the number its
/// characters spell (zero when they spell none).
pub fn value(base: u32, o: &Opd) -> Val {
    match o.tag {
        T_LIT_NUM => {
            let m = mem::rd_i128(o.off);
            let s = mem::rd_u8(o.off + 16);
            Val::Dec(cobolt_decimal::CobolNumeric::new(m, s))
        }
        T_FIG if o.fig == FIG_ZERO => Val::ZERO,
        T_TEMP => crate::state::st().temps[(o.off as usize) % crate::state::NTEMP],
        T_CTEMP => crate::func::ctemp_number(o.off).unwrap_or(Val::ZERO),
        T_ITEM => {
            let loc = locate(base, o);
            codec::load_value(&loc)
        }
        _ => {
            let loc = locate(base, o);
            codec::load_value(&loc)
        }
    }
}
