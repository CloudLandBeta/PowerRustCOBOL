// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Sequential and line-sequential files — **conformance builds only**.
//!
//! A browser module has no files (WASM-010 non-goal): data lives on the
//! backend. The NIST programs, however, write their report to a print file, so
//! the conformance build (WASM-PLAN P6) lowers `OPEN`, `CLOSE`, `READ` and
//! `WRITE` to four host imports that the test host implements over the real
//! file system. Nothing here is linked into a Web or Mobile build: the glue
//! functions these routines import exist only in a conformance module, and the
//! linker refuses a routine whose import has no glue.
//!
//! A file descriptor is a static record:
//!
//! ```text
//!  0 name_addr u32   the ASSIGN name's bytes
//!  4 name_len  u32
//!  8 org       u8    0 sequential, 1 line sequential
//!  9 flags     u8    reserved
//! 12 handle_off u32  region offset of the u32 that holds handle+1 (0 = closed)
//! 16 status_opd u32  operand of the FILE STATUS item, or 0
//! ```

use crate::host;
use crate::mem;
use crate::opd::*;

pub const MODE_INPUT: u32 = 0;
pub const MODE_OUTPUT: u32 = 1;
pub const MODE_IO: u32 = 2;
pub const MODE_EXTEND: u32 = 3;

/// `adv` value meaning "no ADVANCING phrase".
pub const ADV_NONE: u32 = u32::MAX;
pub const ADV_BEFORE: u32 = 1 << 30;
pub const ADV_PAGE: u32 = 1 << 31;

#[derive(Clone, Copy)]
struct FileDesc {
    name_addr: u32,
    name_len: u32,
    org: u8,
    handle_off: u32,
    status_opd: u32,
}

fn desc(a: u32) -> FileDesc {
    let b = mem::bytes(a, 20);
    FileDesc {
        name_addr: u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        name_len: u32::from_le_bytes([b[4], b[5], b[6], b[7]]),
        org: b[8],
        handle_off: u32::from_le_bytes([b[12], b[13], b[14], b[15]]),
        status_opd: u32::from_le_bytes([b[16], b[17], b[18], b[19]]),
    }
}

fn set_status(base: u32, fd: &FileDesc, code: u32) {
    if fd.status_opd == 0 {
        return;
    }
    let loc = locate(base, &parse(fd.status_opd));
    let digits = [b'0' + ((code / 10) % 10) as u8, b'0' + (code % 10) as u8];
    // The status item is PIC XX (or 99): two characters either way.
    let n = loc.len.min(2) as usize;
    mem::bytes_mut(loc.addr, n as u32).copy_from_slice(&digits[..n]);
}

fn handle(base: u32, fd: &FileDesc) -> u32 {
    mem::rd_u32(base + fd.handle_off)
}

/// `OPEN mode file`; the 2-digit status (also stored in FILE STATUS).
pub fn open(base: u32, fda: u32, mode: u32) -> u32 {
    let fd = desc(fda);
    if handle(base, &fd) != 0 {
        set_status(base, &fd, 41);
        return 41;
    }
    // SAFETY: the glue copies the name out of memory and returns a handle.
    let h = unsafe { host::file_open(fd.name_addr, fd.name_len, mode | ((fd.org as u32) << 8)) };
    if h > 0 {
        mem::wr_u32(base + fd.handle_off, h as u32);
        set_status(base, &fd, 0);
        0
    } else {
        let code = (-h) as u32;
        set_status(base, &fd, code);
        code
    }
}

pub fn close(base: u32, fda: u32) -> u32 {
    let fd = desc(fda);
    let h = handle(base, &fd);
    if h == 0 {
        set_status(base, &fd, 42);
        return 42;
    }
    // SAFETY: the handle came from `file_open`.
    let r = unsafe { host::file_close(h) };
    mem::wr_u32(base + fd.handle_off, 0);
    let code = if r < 0 { (-r) as u32 } else { 0 };
    set_status(base, &fd, code);
    code
}

/// `WRITE record` (the record operand names the record area; `adv` is the
/// ADVANCING phrase, see [`ADV_NONE`]).
pub fn write(base: u32, fda: u32, rec: u32, adv: u32) -> u32 {
    let fd = desc(fda);
    let h = handle(base, &fd);
    if h == 0 {
        set_status(base, &fd, 48);
        return 48;
    }
    let loc = locate(base, &parse(rec));
    // SAFETY: the glue copies the record out of memory.
    let r = unsafe { host::file_write(h, loc.addr, loc.len, adv) };
    let code = if r < 0 { (-r) as u32 } else { 0 };
    set_status(base, &fd, code);
    code
}

/// `READ file` into the record area; 0 read, 10 end of file, else a status.
pub fn read(base: u32, fda: u32, rec: u32) -> u32 {
    let fd = desc(fda);
    let h = handle(base, &fd);
    if h == 0 {
        set_status(base, &fd, 47);
        return 47;
    }
    let loc = locate(base, &parse(rec));
    // SAFETY: the glue writes at most `loc.len` bytes at `loc.addr`.
    let n = unsafe { host::file_read(h, loc.addr, loc.len) };
    if n >= 0 {
        // A short record leaves the rest of the area as spaces.
        if (n as u32) < loc.len {
            mem::fill(loc.addr + n as u32, loc.len - n as u32, b' ');
        }
        set_status(base, &fd, 0);
        0
    } else if n == -10 {
        set_status(base, &fd, 10);
        10
    } else {
        let code = (-n) as u32;
        set_status(base, &fd, code);
        code
    }
}
