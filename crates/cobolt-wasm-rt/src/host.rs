// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the routines ask of the outside, and nothing else.
//!
//! These are the functions of the **glue** namespace. The linker resolves them
//! to functions the *generator* emits into the module — not to host imports —
//! because the generator knows which of the module's two memories holds the
//! arena (WASM-017 R8–R9) and Rust code cannot address a second memory. A glue
//! function copies between this crate's staging area in memory 0 and the arena,
//! then calls the real host import of ABI v1.
//!
//! Natively the same functions record what they were asked, so tests can read
//! the commands a routine produced.

#[cfg(target_arch = "wasm32")]
mod imp {
    #[link(wasm_import_module = "glue")]
    extern "C" {
        /// Deliver `len` bytes of command batch staged at `addr` (ABI v1 `flush`).
        pub fn flush(addr: u32, len: u32);
        /// Deliver the batch of `len` bytes at `addr`, which ends in one query;
        /// the answer text is written at `ans_addr` (at most `ans_cap` bytes)
        /// and its length returned, or a negative error as `u32` wrap (ABI v1
        /// `query`).
        pub fn query(addr: u32, len: u32, ans_addr: u32, ans_cap: u32) -> i32;
        /// Send the request frame staged at `addr` (ABI v1 `remote`).
        pub fn remote(addr: u32, len: u32) -> i32;
        /// Report run-time error `code` (see `RtError`) and trap (ABI v1 `abort`);
        /// the glue writes the `ERROR` command with the current `$line`.
        pub fn abort(code: u32);
        pub fn now() -> f64;
        pub fn random() -> f64;
        // Conformance builds only (WASM-ABI-v1 §8): sequential files.
        /// Open the file named by the `len` bytes at `addr`; `mode` is the
        /// OPEN mode in the low byte and the organisation in the next.
        /// Returns a handle >= 1 or a negative file status.
        pub fn file_open(addr: u32, len: u32, mode: u32) -> i32;
        pub fn file_close(handle: u32) -> i32;
        /// Write one record; `adv` encodes ADVANCING (`file::ADV_*`).
        pub fn file_write(handle: u32, addr: u32, len: u32, adv: u32) -> i32;
        /// Read the next record into memory; returns its length, -10 at the
        /// end of the file, or another negative status.
        pub fn file_read(handle: u32, addr: u32, cap: u32) -> i32;
        /// The state of the external switch named by the `len` bytes at `addr`:
        /// 1 on, 0 off, -1 when the host knows no such switch.
        pub fn switch_state(addr: u32, len: u32) -> i32;
        // A desktop module only (ABI A29): the generator imports these from
        // `prn`, the desktop runner's namespace, and passes memory 0's
        // addresses straight through.
        /// One line of the operator's console into `addr` (at most `cap`
        /// bytes), its line end removed: the bytes written, or -10 at the end
        /// of the input.
        pub fn con_read(addr: u32, cap: u32) -> i32;
        /// The environment and the command line: `kind` 0 the variable whose
        /// name is the first `n` bytes at `buf`, 1 argument `n` (from 1), 2
        /// the number of arguments, 3 the arguments joined by one space. The
        /// text goes to `buf` (at most `cap` bytes, over the name) and the
        /// bytes written are returned; kind 2 writes nothing and returns the
        /// count; -1 when there is no such variable or argument.
        pub fn env_get(kind: u32, n: u32, buf: u32, cap: u32) -> i32;
        // A desktop module only (ABI A34): the runner's files, each named by
        // its number in the `prfiles` section; every call answers the file
        // status as a number (`crate::nfile`). `slot` is the address of the
        // file's place in the region of the program instance that holds it.
        /// `OPEN`: the path is the `path_len` bytes at `path`.
        pub fn f_open(id: u32, slot: u32, mode: u32, path: u32, path_len: u32, flags: u32) -> i32;
        pub fn f_close(id: u32, slot: u32, how: u32) -> i32;
        /// `READ`: the record goes to `rec` (at most `cap` bytes); its length
        /// goes to `out`, and at `out + 8` the slot a RELATIVE read delivered.
        pub fn f_read(id: u32, slot: u32, how: u32, kor: u32, key: u32, key_len: i32, rel: i64, rec: u32, cap: u32, out: u32) -> i32;
        /// `WRITE`: at `out` the slot a sequential RELATIVE write was given.
        pub fn f_write(id: u32, slot: u32, rec: u32, len: u32, rel: i64, out: u32) -> i32;
        pub fn f_rewrite(id: u32, slot: u32, rec: u32, len: u32, rel: i64) -> i32;
        pub fn f_delete(id: u32, slot: u32, key: u32, key_len: i32, rel: i64) -> i32;
        pub fn f_start(id: u32, slot: u32, op: u32, kor: u32, key: u32, key_len: i32, rel: i64) -> i32;
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::cell::RefCell;
    use std::vec::Vec;

    /// What a routine asked of the host, in order.
    #[derive(Debug, Clone, PartialEq)]
    pub enum Event {
        Flush(Vec<u8>),
        Query(Vec<u8>),
        Remote(Vec<u8>),
        Abort(u32),
    }

    std::thread_local! {
        pub static LOG: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
        pub static ANSWER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
        pub static NOW: RefCell<f64> = const { RefCell::new(0.0) };
    }

    pub unsafe fn flush(addr: u32, len: u32) {
        let b = crate::mem::bytes(addr, len).to_vec();
        LOG.with(|l| l.borrow_mut().push(Event::Flush(b)));
    }
    pub unsafe fn query(addr: u32, len: u32, ans_addr: u32, ans_cap: u32) -> i32 {
        let b = crate::mem::bytes(addr, len).to_vec();
        LOG.with(|l| l.borrow_mut().push(Event::Query(b)));
        let ans = ANSWER.with(|a| a.borrow().clone());
        // As the generator's glue does: at most `ans_cap` bytes copied, the
        // answer's whole length returned.
        let n = ans.len().min(ans_cap as usize);
        crate::mem::bytes_mut(ans_addr, n as u32).copy_from_slice(&ans[..n]);
        ans.len() as i32
    }
    pub unsafe fn remote(addr: u32, len: u32) -> i32 {
        let b = crate::mem::bytes(addr, len).to_vec();
        LOG.with(|l| l.borrow_mut().push(Event::Remote(b)));
        1
    }
    pub unsafe fn abort(code: u32) {
        LOG.with(|l| l.borrow_mut().push(Event::Abort(code)));
        panic!("rt abort {code}");
    }
    pub unsafe fn file_open(_addr: u32, _len: u32, _mode: u32) -> i32 {
        1
    }
    pub unsafe fn file_close(_h: u32) -> i32 {
        0
    }
    pub unsafe fn file_write(_h: u32, _addr: u32, _len: u32, _adv: u32) -> i32 {
        0
    }
    pub unsafe fn file_read(_h: u32, _addr: u32, _cap: u32) -> i32 {
        -10
    }
    pub unsafe fn switch_state(_addr: u32, _len: u32) -> i32 {
        -1
    }
    pub unsafe fn con_read(_addr: u32, _cap: u32) -> i32 {
        -10
    }
    pub unsafe fn env_get(kind: u32, _n: u32, _buf: u32, _cap: u32) -> i32 {
        if kind == 2 || kind == 3 {
            0
        } else {
            -1
        }
    }
    pub unsafe fn now() -> f64 {
        NOW.with(|n| *n.borrow())
    }
    pub unsafe fn random() -> f64 {
        0.5
    }

    std::thread_local! {
        /// The file imports called on this thread, each with its arguments (A34).
        pub static FILE_CALLS: RefCell<Vec<(&'static str, Vec<i64>)>> = const { RefCell::new(Vec::new()) };
        /// The status the next file import answers, and what a `f_read` delivers: the record and
        /// the slot (-1 none).
        pub static FILE_ANSWER: RefCell<(i32, Vec<u8>, i64)> = const { RefCell::new((0, Vec::new(), -1)) };
    }
    fn file_call(name: &'static str, args: &[i64]) -> i32 {
        FILE_CALLS.with(|c| c.borrow_mut().push((name, args.to_vec())));
        FILE_ANSWER.with(|a| a.borrow().0)
    }
    pub unsafe fn f_open(id: u32, slot: u32, mode: u32, path: u32, path_len: u32, flags: u32) -> i32 {
        let _ = (path, path_len);
        file_call("f_open", &[id as i64, slot as i64, mode as i64, path_len as i64, flags as i64])
    }
    pub unsafe fn f_close(id: u32, slot: u32, how: u32) -> i32 {
        file_call("f_close", &[id as i64, slot as i64, how as i64])
    }
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn f_read(id: u32, slot: u32, how: u32, kor: u32, key: u32, key_len: i32, rel: i64, rec: u32, cap: u32, out: u32) -> i32 {
        let _ = key;
        let code = file_call("f_read", &[id as i64, slot as i64, how as i64, kor as i64, key_len as i64, rel]);
        let (_, bytes, at) = FILE_ANSWER.with(|a| a.borrow().clone());
        if code == 0 {
            let n = bytes.len().min(cap as usize);
            crate::mem::bytes_mut(rec, n as u32).copy_from_slice(&bytes[..n]);
            crate::mem::wr_u32(out, bytes.len() as u32);
            crate::mem::bytes_mut(out + 8, 8).copy_from_slice(&at.to_le_bytes());
        }
        code
    }
    pub unsafe fn f_write(id: u32, slot: u32, rec: u32, len: u32, rel: i64, out: u32) -> i32 {
        let _ = rec;
        let code = file_call("f_write", &[id as i64, slot as i64, len as i64, rel]);
        let at = FILE_ANSWER.with(|a| a.borrow().2);
        crate::mem::bytes_mut(out, 8).copy_from_slice(&at.to_le_bytes());
        code
    }
    pub unsafe fn f_rewrite(id: u32, slot: u32, rec: u32, len: u32, rel: i64) -> i32 {
        let _ = rec;
        file_call("f_rewrite", &[id as i64, slot as i64, len as i64, rel])
    }
    pub unsafe fn f_delete(id: u32, slot: u32, key: u32, key_len: i32, rel: i64) -> i32 {
        let _ = key;
        file_call("f_delete", &[id as i64, slot as i64, key_len as i64, rel])
    }
    pub unsafe fn f_start(id: u32, slot: u32, op: u32, kor: u32, key: u32, key_len: i32, rel: i64) -> i32 {
        let _ = key;
        file_call("f_start", &[id as i64, slot as i64, op as i64, kor as i64, key_len as i64, rel])
    }
}

pub use imp::*;

/// Take and clear the events recorded on this thread (native tests only).
#[cfg(not(target_arch = "wasm32"))]
pub fn take_log() -> std::vec::Vec<Event> {
    LOG.with(|l| core::mem::take(&mut *l.borrow_mut()))
}
