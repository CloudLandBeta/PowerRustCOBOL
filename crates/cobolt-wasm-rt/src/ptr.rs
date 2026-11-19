// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `USAGE POINTER`, `SET … TO ADDRESS OF`, `SET ADDRESS OF` and `NULL` (DESKTOP-WASM-001 T45b).
//!
//! A pointer holds what the interpreter's holds: a number, 1, 2, … in the order the program first
//! takes the address of each item, and 0 for `NULL`. The generator knows every item whose address the
//! program takes, so it lays out the program's table in its region, zero until written:
//!
//! ```text
//! [count] [address of id 1] … [address of id k] [id of item 0] … [id of item k-1]
//! ```
//!
//! `count` is how many ids are handed out; an item's id word is 0 until its address is first taken.
//! `SET ptr TO NULL` and `SET ptr TO ptr-2` are `MOVE`s (the parser makes them so, for both engines).
//! `SET ADDRESS OF item TO …` stores an address in the item's LINKAGE slot, through which every
//! reference to the item already goes.

use crate::codec;
use crate::mem;
use crate::opd;
use crate::val::Val;

/// The number a pointer item holds.
fn read(base: u32, p: u32) -> i64 {
    match opd::value(base, &opd::parse(p)) {
        Val::Dec(n) => (n.mantissa / cobolt_decimal::pow10(n.decimals as u32).unwrap_or(1)) as i64,
        Val::F(f) => f as i64,
    }
}

/// Store `id` in the pointer item `to`.
fn store(base: u32, to: u32, id: i64) {
    let loc = opd::locate(base, &opd::parse(to));
    codec::store_value(&loc, Val::int(id), false, false);
}

/// `id` when the table hands it out, else 0 (`NULL`, or a number no `ADDRESS OF` gave).
fn valid(base: u32, area: u32, id: i64) -> i64 {
    let count = mem::rd_u32(base.wrapping_add(area)) as i64;
    if id >= 1 && id <= count {
        id
    } else {
        0
    }
}

/// `SET ptr TO ADDRESS OF item`: the item's id, handed out the first time its address is taken; then
/// stored in `to`. `k` is the number of items whose address the program takes, `n` this one's place.
#[no_mangle]
pub extern "C" fn rt_ptr_take(base: u32, to: u32, item: u32, area: u32, k: u32, n: u32) {
    let a = base.wrapping_add(area);
    let word = a.wrapping_add(4 + 4 * k + 4 * n);
    let mut id = mem::rd_u32(word);
    if id == 0 {
        id = mem::rd_u32(a) + 1;
        mem::wr_u32(a, id);
        mem::wr_u32(a.wrapping_add(4 * id), opd::locate(base, &opd::parse(item)).addr);
        mem::wr_u32(word, id);
    }
    store(base, to, id as i64);
}

/// `SET ADDRESS OF item TO ptr`: the item's LINKAGE slot (`slot`, a region offset) takes the address
/// the pointer's id stands for, or 0 for `NULL`.
#[no_mangle]
pub extern "C" fn rt_ptr_bind(base: u32, slot: u32, from: u32, area: u32) {
    let a = base.wrapping_add(area);
    let id = valid(base, area, read(base, from));
    let addr = if id == 0 { 0 } else { mem::rd_u32(a.wrapping_add(4 * id as u32)) };
    mem::wr_u32(base.wrapping_add(slot), addr);
}

/// `SET ADDRESS OF item TO ADDRESS OF item-2`: the slot takes `item-2`'s address.
#[no_mangle]
pub extern "C" fn rt_ptr_point(base: u32, slot: u32, item: u32) {
    let addr = opd::locate(base, &opd::parse(item)).addr;
    mem::wr_u32(base.wrapping_add(slot), addr);
}

/// `SET ADDRESS OF item TO NULL`: the slot is empty again.
#[no_mangle]
pub extern "C" fn rt_ptr_unbind(base: u32, slot: u32) {
    mem::wr_u32(base.wrapping_add(slot), 0);
}
