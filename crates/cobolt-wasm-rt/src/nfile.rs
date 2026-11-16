// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Files on disk — **a desktop module only** (ABI A34, DESKTOP-WASM-001 T38).
//!
//! The desktop runner keeps the files: it opens each one with the engine the interpreter would use
//! and answers every verb with the interpreter's `FILE STATUS`. These routines do the program's
//! half of each statement, as the interpreter's `exec_*` code does it: they find the path the
//! `ASSIGN` names, gather the key, the record number and the record's length from the program's
//! data, call the runner's import, and put what it answers back into the program's items — the
//! record area (a short record padded with spaces), the `RECORD … DEPENDING ON` item, the RELATIVE
//! KEY item and the FILE STATUS item. A file is named by its number in the module's `prfiles`
//! section, and every address is in memory 0.
//!
//! A file descriptor is a static record of words:
//!
//! ```text
//!  0 id          the file's number in `prfiles`
//!  4 org         0 sequential, 1 line sequential, 2 relative, 3 indexed
//!  8 flags       bit 0: the access mode is sequential
//! 12 status_opd  operand of the FILE STATUS item, or 0
//! 16 area_off    region offset of the record area
//! 20 area_len    its length: the longest record description
//! 24 path_addr   the ASSIGN literal's bytes, trimmed (when path_opd is 0)
//! 28 path_len
//! 32 path_opd    operand of the data item the ASSIGN names, or 0
//! 36 relkey_opd  operand of the RELATIVE KEY item, or 0
//! 40 depend_opd  operand of the RECORD … DEPENDING ON item of a variable-length file, or 0
//! 44 handle_off  region offset of 8 bytes; the second word is the open mode + 1 (0 when the
//!                file is not open in a mode), which the USE procedures that name a mode read
//! ```
//!
//! A key spec is `[n, (region offset, length) × n]`: the key's value is those bytes joined.

use crate::host;
use crate::mem;
use crate::mv::scratch;
use crate::opd::*;

pub const ORG_SEQUENTIAL: u32 = 0;
pub const ORG_LINE_SEQUENTIAL: u32 = 1;
pub const ORG_RELATIVE: u32 = 2;
pub const ORG_INDEXED: u32 = 3;

/// `flags` bit 0: the file's access mode is sequential.
pub const F_SEQ_ACCESS: u32 = 1;

/// No key bytes, no record number (the imports' `-1`).
const NONE: i32 = -1;

/// Bytes at the start of the scratch place for what an import answers besides its status.
const OUT: u32 = 16;

#[derive(Clone, Copy)]
struct Desc {
    id: u32,
    org: u32,
    flags: u32,
    status_opd: u32,
    area_off: u32,
    area_len: u32,
    path_addr: u32,
    path_len: u32,
    path_opd: u32,
    relkey_opd: u32,
    depend_opd: u32,
    handle_off: u32,
}

fn desc(d: u32) -> Desc {
    let w = |i: u32| mem::rd_u32(d + 4 * i);
    Desc {
        id: w(0),
        org: w(1),
        flags: w(2),
        status_opd: w(3),
        area_off: w(4),
        area_len: w(5),
        path_addr: w(6),
        path_len: w(7),
        path_opd: w(8),
        relkey_opd: w(9),
        depend_opd: w(10),
        handle_off: w(11),
    }
}

/// Two digits into the FILE STATUS item (PIC XX, 99, or a group of two characters).
fn set_status(base: u32, f: &Desc, code: u32) {
    if f.status_opd == 0 {
        return;
    }
    let loc = locate(base, &parse(f.status_opd));
    let digits = [b'0' + ((code / 10) % 10) as u8, b'0' + (code % 10) as u8];
    // Byte by byte: a `copy_from_slice` here would put a panic location, and this file's name, in
    // the read-only data every module carries, and move a browser module's bytes (AC16).
    for (to, d) in mem::bytes_mut(loc.addr, loc.len.min(2)).iter_mut().zip(digits) {
        *to = d;
    }
}

/// The status a runner import answered (it is never negative; a negative one is a broken file).
fn status(r: i32) -> u32 {
    if r < 0 {
        30
    } else {
        r as u32
    }
}

fn rd_i64(addr: u32) -> i64 {
    let b = mem::bytes(addr, 8);
    i64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}

/// As two words, for the reason [`set_status`] gives.
fn wr_i64(addr: u32, v: i64) {
    mem::wr_u32(addr, v as u32);
    mem::wr_u32(addr + 4, (v >> 32) as u32);
}

/// The path the `ASSIGN` names: the value of the data item it names, its trailing spaces removed,
/// or the literal (as the interpreter's `resolve_assign_path`).
fn path(base: u32, f: &Desc) -> (u32, u32) {
    if f.path_opd == 0 {
        return (f.path_addr, f.path_len);
    }
    let loc = locate(base, &parse(f.path_opd));
    let b = mem::bytes(loc.addr, loc.len);
    let n = b.iter().rposition(|c| !c.is_ascii_whitespace()).map_or(0, |i| i + 1);
    (loc.addr, n as u32)
}

/// A record number from the item `opd`: its integer value, a negative one as 0 (the interpreter's
/// `relative_key_of(…).unwrap_or(0)`); 0 when there is no item.
fn number(base: u32, opd: u32) -> i64 {
    if opd == 0 {
        0
    } else {
        crate::rt_int(base, opd).max(0)
    }
}

/// The record number a verb on a RELATIVE file carries; [`NONE`] for the other organisations.
fn rel_of(base: u32, f: &Desc) -> i64 {
    if f.org == ORG_RELATIVE {
        number(base, f.relkey_opd)
    } else {
        NONE as i64
    }
}

/// The value the key spec `ks` names, joined at `at`: its length, or [`NONE`] for no key.
fn key(base: u32, ks: u32, at: u32, cap: u32) -> i32 {
    if ks == 0 {
        return NONE;
    }
    let n = mem::rd_u32(ks);
    let mut done = 0u32;
    for i in 0..n {
        let off = mem::rd_u32(ks + 4 + 8 * i);
        let len = mem::rd_u32(ks + 8 + 8 * i).min(cap - done);
        mem::copy(base + off, at + done, len);
        done += len;
    }
    done as i32
}

/// The record a `WRITE` or `REWRITE` sends, `len` bytes long: the record area itself, or — when the
/// length the program asked for is longer than the area — the area followed by spaces, in the
/// scratch place after `OUT`.
fn record(base: u32, f: &Desc, len: u32) -> (u32, u32) {
    let area = base + f.area_off;
    if len <= f.area_len {
        return (area, len);
    }
    let (s, cap) = scratch();
    let len = len.min(cap - OUT);
    mem::copy(area, s + OUT, f.area_len);
    mem::fill(s + OUT + f.area_len, len - f.area_len, b' ');
    (s + OUT, len)
}

/// The length a `WRITE` (or a sequential file's `REWRITE`) gives the record: `RECORD … DEPENDING
/// ON`'s value when the file has the phrase, else `declared`, the named record description's.
fn write_len(base: u32, f: &Desc, declared: u32) -> u32 {
    if f.depend_opd == 0 {
        declared
    } else {
        crate::rt_int(base, f.depend_opd).max(0) as u32
    }
}

/// `OPEN mode file`; `flags` bit 0 for `WITH LOCK` or `SHARING WITH NO OTHER`.
pub fn open(base: u32, d: u32, mode: u32, flags: u32) -> u32 {
    let f = desc(d);
    let (addr, len) = path(base, &f);
    // SAFETY: the runner reads `len` bytes of memory 0 at `addr`.
    let code = status(unsafe { host::f_open(f.id, base + f.handle_off, mode, addr, len, flags) });
    // The mode a USE procedure that names one matches: kept by an OPEN of a file already open (41)
    // or closed WITH LOCK (38), forgotten by a keyed file whose engine refused it.
    if code != 41 && code != 38 {
        let forgotten = f.org >= ORG_RELATIVE && code >= 10 && code != 93;
        mem::wr_u32(base + f.handle_off + 4, if forgotten { 0 } else { mode + 1 });
    }
    set_status(base, &f, code);
    code
}

/// `CLOSE file`; `how` 0 plain, 1 `WITH LOCK`, 2 `REEL`/`UNIT`.
pub fn close(base: u32, d: u32, how: u32) -> u32 {
    let f = desc(d);
    // SAFETY: no memory crosses.
    let code = status(unsafe { host::f_close(f.id, base + f.handle_off, how) });
    set_status(base, &f, code);
    code
}

/// `READ file`; `how` is the imports' bits (keyed, previous, no lock), `kor` the key of reference
/// and `ks` the spec of its value (0 for none).
pub fn read(base: u32, d: u32, how: u32, kor: u32, ks: u32) -> u32 {
    let f = desc(d);
    let (s, cap) = scratch();
    let key_len = key(base, ks, s + OUT, cap - OUT);
    mem::wr_u32(s, 0);
    wr_i64(s + 8, NONE as i64);
    let rec = base + f.area_off;
    // SAFETY: the runner reads the key at `s + OUT`, writes at most `area_len` bytes at `rec` and
    // 16 at `s`.
    let code = status(unsafe { host::f_read(f.id, base + f.handle_off, how, kor, s + OUT, key_len, rel_of(base, &f), rec, f.area_len, s) });
    if code == 0 {
        let n = mem::rd_u32(s);
        // A short record leaves the rest of the area as spaces.
        if n < f.area_len {
            mem::fill(rec + n, f.area_len - n, b' ');
        }
        // `RECORD … DEPENDING ON` reads back as well: the item holds the length read.
        if f.depend_opd != 0 {
            crate::rt_set_int(base, f.depend_opd, n as i64);
        }
    }
    // A sequential RELATIVE read tells the program where the record sits.
    let at = rd_i64(s + 8);
    if at >= 0 && f.relkey_opd != 0 {
        crate::rt_set_int(base, f.relkey_opd, at);
    }
    set_status(base, &f, code);
    code
}

/// `WRITE record`, `declared` being the length of the record description the statement names.
pub fn write(base: u32, d: u32, declared: u32) -> u32 {
    let f = desc(d);
    let (rec, len) = record(base, &f, write_len(base, &f, declared));
    // In the sequential access mode a RELATIVE file gives the record the next slot and says which.
    let random_rel = f.org == ORG_RELATIVE && f.flags & F_SEQ_ACCESS == 0;
    let rel = if random_rel { number(base, f.relkey_opd) } else { NONE as i64 };
    let (s, _) = scratch();
    wr_i64(s, NONE as i64);
    // SAFETY: the runner reads `len` bytes at `rec` and writes 8 at `s`.
    let code = status(unsafe { host::f_write(f.id, base + f.handle_off, rec, len, rel, s) });
    if code == 0 && f.org == ORG_RELATIVE && !random_rel {
        let at = rd_i64(s);
        if at >= 0 && f.relkey_opd != 0 {
            crate::rt_set_int(base, f.relkey_opd, at);
        }
    }
    set_status(base, &f, code);
    code
}

/// `REWRITE record`: an INDEXED file's whole first record description (`declared`), another
/// organisation's at the length a `WRITE` would give it.
pub fn rewrite(base: u32, d: u32, declared: u32) -> u32 {
    let f = desc(d);
    let len = if f.org == ORG_INDEXED { declared } else { write_len(base, &f, declared) };
    let (rec, len) = record(base, &f, len);
    // SAFETY: the runner reads `len` bytes at `rec`.
    let code = status(unsafe { host::f_rewrite(f.id, base + f.handle_off, rec, len, rel_of(base, &f)) });
    set_status(base, &f, code);
    code
}

/// `DELETE file`; `ks` the spec of the RECORD KEY's value (0 when the file declares none).
pub fn delete(base: u32, d: u32, ks: u32) -> u32 {
    let f = desc(d);
    let (s, cap) = scratch();
    let key_len = key(base, ks, s + OUT, cap - OUT);
    // SAFETY: the runner reads the key at `s + OUT`.
    let code = status(unsafe { host::f_delete(f.id, base + f.handle_off, s + OUT, key_len, rel_of(base, &f)) });
    set_status(base, &f, code);
    code
}

/// `START file KEY op …`; `rel_opd` the item whose value a RELATIVE file's START compares (the KEY
/// phrase's, else the RELATIVE KEY).
pub fn start(base: u32, d: u32, op: u32, kor: u32, ks: u32, rel_opd: u32) -> u32 {
    let f = desc(d);
    let (s, cap) = scratch();
    let key_len = key(base, ks, s + OUT, cap - OUT);
    let rel = if f.org == ORG_RELATIVE { number(base, rel_opd) } else { NONE as i64 };
    // SAFETY: the runner reads the key at `s + OUT`.
    let code = status(unsafe { host::f_start(f.id, base + f.handle_off, op, kor, s + OUT, key_len, rel) });
    set_status(base, &f, code);
    code
}
