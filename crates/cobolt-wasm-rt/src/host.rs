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
        /// `UNLOCK`, `COMMIT`, `ROLLBACK` (A35): `op` is `verb::TX_*`; the file is `NONE` for the last two.
        pub fn f_tx(op: u32, id: u32, slot: u32) -> i32;
        // A desktop module only (ABI A39): a built-in's text file or picture, named by its path.
        /// `APPEND-FILE` (`append` 1) or `WRITE-FILE` (0): the `text_len` bytes at `text` as one line
        /// of the file whose path is the `path_len` bytes at `path`. The answer is the length of why
        /// it failed, written at `buf` (at most `cap` bytes): 0 when the line was written.
        pub fn t_write(append: u32, path: u32, path_len: u32, text: u32, text_len: u32, buf: u32, cap: u32) -> i32;
        /// `IMAGE-FIT`: the picture at `src` fitted to `width` × `height` and written to `dst`; the
        /// status word goes to `buf` (at most `cap` bytes) and its length is returned.
        pub fn img_fit(src: u32, src_len: u32, dst: u32, dst_len: u32, width: i64, height: i64, buf: u32, cap: u32) -> i32;
        /// `IMAGE-INFO`: at `buf` the width and the height (two little-endian words), then the format
        /// and the status word with one space between them (at most `cap - 8` bytes, whose length
        /// is returned).
        pub fn img_info(path: u32, path_len: u32, buf: u32, cap: u32) -> i32;
        // A desktop module only (ABI A41): an `EXEC SQL` statement, run by the runner.
        /// Statement `stmt` of the `prsql` section, with the `len` bytes of cell values at `buf`; the
        /// answer is written over them (at most `cap` bytes) and its length returned, or a negative
        /// number when the runner could not run the statement.
        pub fn sql_exec(stmt: u32, buf: u32, len: u32, cap: u32) -> i32;
        // A desktop module only (ABI A42): a SQL built-in, run by the runner over its connections.
        /// Operation `op` (`crate::sql::db`) with the handle, the column and the `text_len` bytes at
        /// `text`; the receivers' values are written at `buf` (at most `cap` bytes) and their length
        /// returned, or a negative number when the runner could not run the call.
        pub fn sql_db(op: u32, handle: i64, number: i64, text: u32, text_len: u32, buf: u32, cap: u32) -> i32;
        // A desktop module only (ABI A44): `COBOL::SqlDatabase`, run by the runner over its connections.
        /// Operation `op` (`crate::sql::obj`) on the member named by the `member_len` bytes at `member`,
        /// with the `text_len` bytes at `text`; the answer is written at `buf` when it fits in `cap`
        /// bytes, and its whole length returned, or a negative number when the runner could not run it.
        pub fn sql_obj(op: u32, member: u32, member_len: u32, text: u32, text_len: u32, buf: u32, cap: u32) -> i32;
        // A desktop module only (ABI A45): an HTTP built-in, run by the runner with the interpreter's client.
        /// Operation `op` (`crate::deskio::http`) with the `a_len` bytes at `a` and the `b_len` bytes at
        /// `b`; the answer's text is written at `buf` (at most `cap` bytes: its first, or with `TAIL` the
        /// last of it without its trailing spaces), and its number (bits 32 to 47) and whole length (bits
        /// 0 to 31) returned, or -1.
        pub fn http_call(op: u32, a: u32, a_len: u32, b: u32, b_len: u32, buf: u32, cap: u32) -> i64;
        // A desktop module only (ABI A56): `COBOL::"PARTIQL"` and its two siblings, run by the runner.
        /// Operation `op` (`crate::pq`) with the `req_len` bytes of the request at `req`; the
        /// answer's text is written at `buf` (at most `cap` bytes: its first, or with `TAIL` the last of
        /// it without its trailing spaces), and its number (bits 32 to 62) and whole length (bits 0 to
        /// 31) returned, or -1.
        pub fn pq_file(op: u32, req: u32, req_len: u32, buf: u32, cap: u32) -> i64;
        // A Backend module only (ABI A60): the verbs of the call being answered, run by the Backend runner.
        /// `WS-FAIL`: the failure `code` (a number the runner holds to 1 to 7999), the `msg_len` bytes at `msg` and the
        /// `detail_len` at `detail` (0: none). Answers 0; a run-time error is a trap.
        pub fn ws_fail(code: f64, msg: u32, msg_len: u32, detail: u32, detail_len: u32) -> i32;
        /// `WS-PROGRESS`: `percent` and the `msg_len` bytes at `msg`. Answers 1 when the call must end (its caller asked to
        /// cancel), else 0.
        pub fn ws_progress(percent: f64, msg: u32, msg_len: u32) -> i32;
        /// `WS-CANCELLED`: 1 when the caller asked to cancel, else 0.
        pub fn ws_cancelled() -> i32;
        /// `WS-PAUSE`: wait `ms` milliseconds, ending early when the caller asks to cancel. Answers 0; a call the server
        /// stops is a trap.
        pub fn ws_pause(ms: f64) -> i32;
        /// `WS-EMIT`: the `len` bytes of JSON at `buf` as the stream's next item, or, when `len` is negative, the negated
        /// code of the mapping that could not write the group. Answers 0 to go on and 1 when the call must end.
        pub fn ws_emit(buf: u32, len: i32) -> i32;
        /// `WS-CLAIM`: the value of the claim named by the `name_len` bytes at `name`, written at `buf` (at most `cap`
        /// bytes); its length is returned.
        pub fn ws_claim(name: u32, name_len: u32, buf: u32, cap: u32) -> i32;
        /// `WS-FILE-PATH`: the path of the file the id (`id_len` bytes at `id`) names, then the status word, written at
        /// `buf` (at most `cap` bytes); returns the status word's length (bits 32 to 47) and the path's (bits 0 to 31), or -1.
        pub fn ws_file_path(id: u32, id_len: u32, buf: u32, cap: u32) -> i64;
        /// `WS-FILE-PUBLISH`: the id of the file at the path (`path_len` bytes at `path`), then the status word, as above.
        pub fn ws_file_publish(path: u32, path_len: u32, buf: u32, cap: u32) -> i64;
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
    pub unsafe fn f_tx(op: u32, id: u32, slot: u32) -> i32 {
        file_call("f_tx", &[op as i64, id as i32 as i64, slot as i32 as i64])
    }

    std::thread_local! {
        /// What the built-ins' imports were given on this thread (A39): the import, then its texts.
        pub static BUILTIN_CALLS: RefCell<Vec<(&'static str, Vec<Vec<u8>>, Vec<i64>)>> = const { RefCell::new(Vec::new()) };
        /// What the next one answers at `buf`.
        pub static BUILTIN_ANSWER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    }
    fn builtin_call(name: &'static str, texts: &[(u32, u32)], numbers: &[i64], buf: u32, cap: u32) -> i32 {
        let texts = texts.iter().map(|&(a, n)| crate::mem::bytes(a, n).to_vec()).collect();
        BUILTIN_CALLS.with(|c| c.borrow_mut().push((name, texts, numbers.to_vec())));
        let ans = BUILTIN_ANSWER.with(|a| a.borrow().clone());
        let n = ans.len().min(cap as usize);
        crate::mem::bytes_mut(buf, n as u32).copy_from_slice(&ans[..n]);
        n as i32
    }
    pub unsafe fn t_write(append: u32, path: u32, path_len: u32, text: u32, text_len: u32, buf: u32, cap: u32) -> i32 {
        builtin_call("t_write", &[(path, path_len), (text, text_len)], &[append as i64], buf, cap)
    }
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn img_fit(src: u32, src_len: u32, dst: u32, dst_len: u32, width: i64, height: i64, buf: u32, cap: u32) -> i32 {
        builtin_call("img_fit", &[(src, src_len), (dst, dst_len)], &[width, height], buf, cap)
    }
    /// The answer here is the whole record (the two words included); the import returns the length after them.
    pub unsafe fn img_info(path: u32, path_len: u32, buf: u32, cap: u32) -> i32 {
        (builtin_call("img_info", &[(path, path_len)], &[], buf, cap) - 8).max(0)
    }

    std::thread_local! {
        /// The `EXEC SQL` statements run on this thread (A41): the statement and the cell values sent.
        pub static SQL_CALLS: RefCell<Vec<(u32, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
        /// What the next one answers.
        pub static SQL_ANSWER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    }
    pub unsafe fn sql_exec(stmt: u32, buf: u32, len: u32, cap: u32) -> i32 {
        SQL_CALLS.with(|c| c.borrow_mut().push((stmt, crate::mem::bytes(buf, len).to_vec())));
        let ans = SQL_ANSWER.with(|a| a.borrow().clone());
        if ans.len() > cap as usize {
            return -1;
        }
        crate::mem::bytes_mut(buf, ans.len() as u32).copy_from_slice(&ans);
        ans.len() as i32
    }
    /// A SQL built-in (A42): recorded with the built-ins' imports, as `("sql_db", [text], [op, handle, number])`.
    pub unsafe fn sql_db(op: u32, handle: i64, number: i64, text: u32, text_len: u32, buf: u32, cap: u32) -> i32 {
        builtin_call("sql_db", &[(text, text_len)], &[op as i64, handle, number], buf, cap)
    }
    /// `COBOL::SqlDatabase` (A44): recorded with the built-ins' imports, as `("sql_obj", [member, text], [op])`.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn sql_obj(op: u32, member: u32, member_len: u32, text: u32, text_len: u32, buf: u32, cap: u32) -> i32 {
        builtin_call("sql_obj", &[(member, member_len), (text, text_len)], &[op as i64], buf, cap)
    }
    std::thread_local! {
        /// The number the next `http_call` answers with the text in `BUILTIN_ANSWER` (A45).
        pub static HTTP_NUMBER: RefCell<u16> = const { RefCell::new(0) };
    }
    /// An HTTP built-in (A45): recorded with the built-ins' imports, as `("http_call", [a, b], [op, cap])`;
    /// the text written is the first `cap` bytes of `BUILTIN_ANSWER`, or with `TAIL` the last of it
    /// without its trailing spaces.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn http_call(op: u32, a: u32, a_len: u32, b: u32, b_len: u32, buf: u32, cap: u32) -> i64 {
        let texts = [(a, a_len), (b, b_len)].iter().map(|&(p, n)| crate::mem::bytes(p, n).to_vec()).collect();
        BUILTIN_CALLS.with(|c| c.borrow_mut().push(("http_call", texts, vec![op as i64, cap as i64])));
        let mut ans = BUILTIN_ANSWER.with(|a| a.borrow().clone());
        let tail = op & crate::deskio::http::TAIL != 0;
        if tail {
            ans.truncate(ans.iter().rposition(|&c| c != b' ').map_or(0, |e| e + 1));
        }
        let n = ans.len().min(cap as usize);
        let from = if tail { ans.len() - n } else { 0 };
        crate::mem::bytes_mut(buf, n as u32).copy_from_slice(&ans[from..from + n]);
        let number = HTTP_NUMBER.with(|v| *v.borrow());
        ((u64::from(number) << 32) | ans.len() as u64) as i64
    }
    std::thread_local! {
        /// The number the next `pq_file` answers with the text in `BUILTIN_ANSWER` (A56).
        pub static PQ_NUMBER: RefCell<u32> = const { RefCell::new(0) };
    }
    /// `COBOL::"PARTIQL"` and its two siblings (A56): recorded with the built-ins' imports, as
    /// `("pq_file", [the request's words, then each text it names], [op, cap])`; the text written is
    /// the first `cap` bytes of `BUILTIN_ANSWER`, or with `TAIL` the last of it without its trailing
    /// spaces.
    pub unsafe fn pq_file(op: u32, req: u32, req_len: u32, buf: u32, cap: u32) -> i64 {
        let words: Vec<u32> = (0..req_len / 4).map(|i| crate::mem::rd_u32(req + 4 * i)).collect();
        let mut texts = vec![crate::mem::bytes(req, req_len).to_vec()];
        let nfiles = words[0] as usize;
        let mut k = 1 + 2 * nfiles;
        let text = |k: usize| crate::mem::bytes(words[k], words[k + 1]).to_vec();
        texts.push(text(k));
        k += 2;
        if op & !crate::pq::TAIL == crate::pq::RUN {
            texts.push(text(k));
            let nargs = words[k + 2] as usize;
            for i in 0..nargs {
                texts.push(text(k + 4 + 3 * i));
            }
        }
        BUILTIN_CALLS.with(|c| c.borrow_mut().push(("pq_file", texts, vec![op as i64, cap as i64])));
        let mut ans = BUILTIN_ANSWER.with(|a| a.borrow().clone());
        let tail = op & crate::pq::TAIL != 0;
        if tail {
            ans.truncate(ans.iter().rposition(|&c| c != b' ').map_or(0, |e| e + 1));
        }
        let n = ans.len().min(cap as usize);
        let from = if tail { ans.len() - n } else { 0 };
        crate::mem::bytes_mut(buf, n as u32).copy_from_slice(&ans[from..from + n]);
        let number = PQ_NUMBER.with(|v| *v.borrow());
        ((u64::from(number) << 32) | ans.len() as u64) as i64
    }
    std::thread_local! {
        /// What the next `ws_progress`, `ws_emit` and `ws_pause` answer, and the claim and the file words the others write
        /// (A60): the verbs of a Backend, recorded with the built-ins' imports as `("ws_…", [texts], [numbers])`.
        pub static WS_STOP: RefCell<i32> = const { RefCell::new(0) };
        pub static WS_CANCEL_ASKED: RefCell<i32> = const { RefCell::new(0) };
        pub static WS_NUMBERS: RefCell<Vec<f64>> = const { RefCell::new(Vec::new()) };
        pub static WS_PAIR: RefCell<(Vec<u8>, Vec<u8>)> = const { RefCell::new((Vec::new(), Vec::new())) };
    }
    pub unsafe fn ws_fail(code: f64, msg: u32, msg_len: u32, detail: u32, detail_len: u32) -> i32 {
        WS_NUMBERS.with(|n| n.borrow_mut().push(code));
        builtin_call("ws_fail", &[(msg, msg_len), (detail, detail_len)], &[], 0, 0);
        0
    }
    pub unsafe fn ws_progress(percent: f64, msg: u32, msg_len: u32) -> i32 {
        WS_NUMBERS.with(|n| n.borrow_mut().push(percent));
        builtin_call("ws_progress", &[(msg, msg_len)], &[], 0, 0);
        WS_STOP.with(|s| *s.borrow())
    }
    pub unsafe fn ws_cancelled() -> i32 {
        WS_CANCEL_ASKED.with(|s| *s.borrow())
    }
    pub unsafe fn ws_pause(ms: f64) -> i32 {
        WS_NUMBERS.with(|n| n.borrow_mut().push(ms));
        0
    }
    pub unsafe fn ws_emit(buf: u32, len: i32) -> i32 {
        let n = len.max(0) as u32;
        builtin_call("ws_emit", &[(buf, n)], &[len as i64], 0, 0);
        WS_STOP.with(|s| *s.borrow())
    }
    pub unsafe fn ws_claim(name: u32, name_len: u32, buf: u32, cap: u32) -> i32 {
        builtin_call("ws_claim", &[(name, name_len)], &[], buf, cap)
    }
    /// The answer of a file verb: the text, then the status word (`WS_PAIR`).
    fn ws_pair(name: &'static str, at: u32, len: u32, buf: u32, cap: u32) -> i64 {
        builtin_call(name, &[(at, len)], &[], 0, 0);
        let (text, word) = WS_PAIR.with(|p| p.borrow().clone());
        let mut all = text.clone();
        all.extend_from_slice(&word);
        let n = all.len().min(cap as usize);
        crate::mem::bytes_mut(buf, n as u32).iter_mut().zip(all.iter()).for_each(|(d, s)| *d = *s);
        ((word.len() as u64) << 32 | text.len() as u64) as i64
    }
    pub unsafe fn ws_file_path(id: u32, id_len: u32, buf: u32, cap: u32) -> i64 {
        ws_pair("ws_file_path", id, id_len, buf, cap)
    }
    pub unsafe fn ws_file_publish(path: u32, path_len: u32, buf: u32, cap: u32) -> i64 {
        ws_pair("ws_file_publish", path, path_len, buf, cap)
    }
}

pub use imp::*;

/// Take and clear the events recorded on this thread (native tests only).
#[cfg(not(target_arch = "wasm32"))]
pub fn take_log() -> std::vec::Vec<Event> {
    LOG.with(|l| core::mem::take(&mut *l.borrow_mut()))
}
