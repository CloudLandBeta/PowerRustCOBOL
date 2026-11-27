// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Backend's verbs (ABI A60, spec 114 M5): `COBOL::"WS-FAIL"`, `"WS-PROGRESS"`, `"WS-CANCELLED"`, `"WS-PAUSE"`,
//! `"WS-EMIT"`, `"WS-CLAIM"`, `"WS-FILE-PATH"` and `"WS-FILE-PUBLISH"` of a program a Backend exports.
//!
//! Every verb acts on the call the program is answering, which only the runner that serves the call knows. The runner
//! applies the rules the interpreter applies (`cobolt_wire::verbs`: how long a message may be, what a percentage rounds to,
//! how long a pause lasts, which words a refusal uses), so nothing here decides a rule: each routine reads its operands as the
//! interpreter's `eval_call_arg` reads them, hands them to the import, and puts what the runner answered into the receivers
//! the way `set_str` does.
//!
//! This file has no text of its own and nothing that can panic (no indexing, no slice copy): the runtime's data segment is
//! what it was before this file, so the modules the generator already writes are the bytes they were (spec 114 M2).

use crate::deskio::{answer_area, arg_text, put_text};
use crate::host;
use crate::mem;
use crate::opd::*;
use crate::state::{st, FRAME_SIZE, WS_FRAME};
use crate::val::Val;

/// Operand `raw` as the interpreter's `as_f64` reads a value: a number as it is, a text as `parse::<f64>` after `trim`
/// (0 when it is not one).
fn number_f64(base: u32, raw: u32) -> f64 {
    let o = parse(raw);
    let numeric = match o.tag {
        T_LIT_NUM | T_TEMP => Some(value(base, &o)),
        T_LIT_ALNUM | T_ALL | T_CTEMP | T_FIG => None,
        _ => {
            let l = locate(base, &o);
            l.fd.is_numeric().then(|| crate::codec::load_value(&l))
        }
    };
    match numeric {
        Some(Val::Dec(d)) => d.to_f64(),
        Some(Val::F(f)) => f,
        None => {
            let (a, l) = arg_text(base, raw, 0);
            text_f64(mem::bytes(a, l))
        }
    }
}

/// A text as `str::trim` then `parse::<f64>` read it, 0 when it is not a number.
fn text_f64(b: &[u8]) -> f64 {
    let start = b.iter().position(|c| !is_space(*c)).unwrap_or(b.len());
    let end = b.iter().rposition(|c| !is_space(*c)).map_or(start, |e| e + 1);
    let t = b.get(start..end).unwrap_or(&[]);
    core::str::from_utf8(t).ok().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0)
}

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
}

/// `COBOL::"WS-FAIL" ( code message [detail] )`: the failure the program reports for this call. `detail` is 0 when the
/// statement has none. The runner ends the call with it; the caller of this routine ends the run.
pub fn fail(base: u32, code: u32, message: u32, detail: u32) {
    let code = number_f64(base, code);
    let (ma, ml) = arg_text(base, message, 0);
    let (da, dl) = if detail != 0 { arg_text(base, detail, 1) } else { (0, 0) };
    // SAFETY: the runner reads the two texts and answers 0.
    let _ = unsafe { host::ws_fail(code, ma, ml, da, dl) };
}

/// `COBOL::"WS-PROGRESS" ( percent [message] )`: 1 when the call must end because its caller asked to cancel.
pub fn progress(base: u32, percent: u32, message: u32) -> i32 {
    let p = number_f64(base, percent);
    let (ma, ml) = if message != 0 { arg_text(base, message, 0) } else { (0, 0) };
    // SAFETY: the runner reads the message.
    unsafe { host::ws_progress(p, ma, ml) }
}

/// `COBOL::"WS-CANCELLED" ( flag )`: `Y` in `flag` when the caller asked to cancel, else `N`.
pub fn cancelled(base: u32, flag: u32) {
    // SAFETY: no memory is shared.
    let asked = unsafe { host::ws_cancelled() };
    let letter = [if asked != 0 { 89u8 } else { 78u8 }];
    put_text(base, flag, &letter);
}

/// `COBOL::"WS-PAUSE" ( milliseconds )`: wait, ending early when the caller asked to cancel.
pub fn pause(base: u32, milliseconds: u32) {
    let ms = number_f64(base, milliseconds);
    // SAFETY: no memory is shared; a call the server stops traps in the import.
    let _ = unsafe { host::ws_pause(ms) };
}

/// `COBOL::"WS-EMIT" ( group )`: the group as JSON, sent as the stream's next item. 1 when the call must end because
/// nobody wants more items or the caller asked to cancel. A group that cannot be written as JSON goes to the runner as
/// the negated code of the mapping, which says why in the interpreter's words.
pub fn emit(base: u32, desc: u32, group: u32) -> i32 {
    let addr = locate(base, &parse(group)).addr;
    let at = st().ws + WS_FRAME;
    // Through the exported routine, not `json::encode`: a second caller of `encode` changes how the linker places `encode_into`
    // in the routine blob, which moves the address of every call of every module (the player's fixtures, the AC16 pin).
    let n = crate::rt_json_encode(desc, addr, at, FRAME_SIZE);
    // SAFETY: the runner reads `n` bytes of JSON at `at`.
    unsafe { host::ws_emit(at, n) }
}

/// `COBOL::"WS-CLAIM" ( name value )`: the caller's claim `name` in `value`, spaces when the caller has none.
pub fn claim(base: u32, name: u32, value: u32) {
    let (na, nl) = arg_text(base, name, 0);
    let (buf, cap) = answer_area();
    // SAFETY: the runner reads the name and writes at most `cap` bytes at `buf`.
    let n = unsafe { host::ws_claim(na, nl, buf, cap) };
    if n < 0 {
        return;
    }
    put_text(base, value, mem::bytes(buf, (n as u32).min(cap)));
}

/// What a file verb's answer holds at the answer area: the text the verb answers (the path, or the id), then the status
/// word, whose length is the answer's number.
fn put_pair(base: u32, text: u32, status: u32, buf: u32, cap: u32, got: i64) {
    if got < 0 {
        return;
    }
    let n = ((got as u64 & 0xFFFF_FFFF) as u32).min(cap);
    let word = (((got as u64) >> 32) & 0xFFFF) as u32;
    let word = word.min(cap - n);
    put_text(base, text, mem::bytes(buf, n));
    put_text(base, status, mem::bytes(buf + n, word));
}

/// `COBOL::"WS-FILE-PATH" ( file-id path status )`: where the file the caller may reach is on disk, and `OK`, `NOFILE` or
/// `NOBACKEND`.
pub fn file_path(base: u32, id: u32, path: u32, status: u32) {
    let (ia, il) = arg_text(base, id, 0);
    let (buf, cap) = answer_area();
    // SAFETY: the runner reads the id and writes at most `cap` bytes at `buf`.
    let got = unsafe { host::ws_file_path(ia, il, buf, cap) };
    put_pair(base, path, status, buf, cap, got);
}

/// `COBOL::"WS-FILE-PUBLISH" ( path file-id status )`: the id the caller can fetch the file at `path` by, and `OK`,
/// `NOFILE` or `NOBACKEND`.
pub fn file_publish(base: u32, path: u32, id: u32, status: u32) {
    let (pa, pl) = arg_text(base, path, 0);
    let (buf, cap) = answer_area();
    // SAFETY: the runner reads the path and writes at most `cap` bytes at `buf`.
    let got = unsafe { host::ws_file_publish(pa, pl, buf, cap) };
    put_pair(base, id, status, buf, cap, got);
}
