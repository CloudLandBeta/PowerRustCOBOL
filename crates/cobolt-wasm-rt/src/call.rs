// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `CALL`: binding the arguments to the callee's LINKAGE records, and finding
//! a program by name.
//!
//! A LINKAGE record has no storage of its own. The callee's references reach it
//! through a four-byte **slot** in the region (`OF_LINK`), and the caller fills
//! the slot with the address of the argument: `BY REFERENCE` is the argument's
//! own address, `BY CONTENT` and `BY VALUE` the address of a copy the caller
//! made for the call (an item's bytes, or a literal moved into a record shaped
//! like the callee's parameter). The callee therefore sees the caller's bytes
//! through its own description of them, which is what the standard means by
//! the two descriptions covering the same characters.

use crate::mem;
use crate::mv;
use crate::opd::*;
use crate::state::err;

/// `CALL … USING BY REFERENCE arg`: the slot receives the argument's address.
pub fn bind(base: u32, slot: u32, src: u32) {
    let loc = locate(base, &parse(src));
    mem::wr_u32(base + slot, loc.addr);
}

/// `CALL … USING BY CONTENT arg`: copy into the record at region offset `tmp`
/// (`tmp_len` bytes), then point the slot at it. An item is copied byte for
/// byte; a literal or a computed value is moved into `dst`, a record laid out
/// like the callee's parameter, so that it arrives in the callee's format.
pub fn bind_content(base: u32, slot: u32, src: u32, tmp: u32, dst: u32, tmp_len: u32) {
    let s = parse(src);
    let at = base + tmp;
    match s.tag {
        T_ITEM | T_CTEMP => {
            let l = locate(base, &s);
            let n = l.len.min(tmp_len);
            mem::copy(l.addr, at, n);
            if tmp_len > n {
                mem::fill(at + n, tmp_len - n, b' ');
            }
        }
        _ => {
            let snd = mv::sender(base, &s);
            mv::move_to(base, &snd, &parse(dst));
        }
    }
    mem::wr_u32(base + slot, at);
}

/// The index of the program named by operand `name` in `table`
/// (`[count][addr, len] × count`, upper case), or -1. Case does not matter and
/// trailing spaces do not count.
pub fn find_program(base: u32, name: u32, table: u32) -> i32 {
    let l = locate(base, &parse(name));
    let given = mem::bytes(l.addr, l.len);
    let start = given.iter().position(|&b| b != b' ').unwrap_or(given.len());
    let end = given.iter().rposition(|&b| b != b' ').map_or(start, |i| i + 1);
    let want = &given[start..end.max(start)];
    let n = mem::rd_u32(table);
    for i in 0..n {
        let e = table + 4 + i * 8;
        let (a, len) = (mem::rd_u32(e), mem::rd_u32(e + 4));
        let have = mem::bytes(a, len);
        if have.len() == want.len() && have.iter().zip(want).all(|(h, w)| *h == w.to_ascii_uppercase()) {
            return i as i32;
        }
    }
    -1
}

/// A `CALL` that names no program of the module and has no `ON EXCEPTION`.
pub fn not_found() -> ! {
    crate::fail(err::PROGRAM_NOT_FOUND)
}
