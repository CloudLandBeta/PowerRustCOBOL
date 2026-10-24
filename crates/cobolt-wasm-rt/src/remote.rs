// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `CALL … WITH WEBSOCKET LINKAGE` in a browser module (WASM-005 R1, R9;
//! WASM-010 T16).
//!
//! The statement is two halves around a suspension. [`request`] encodes the
//! `USING` groups (`crate::json`), builds the request frame and hands it to the
//! host (`remote`, ABI v1 §3); the task then returns `SUSPENDED`. When the
//! host resumes it, `pr_resume` has copied the answer frame into the workspace
//! and [`result`] reads it: the by-reference groups and the `RETURNING` group
//! are checked first and written only when all of them fit (R3).
//!
//! The request frame is the body of `cobolt_wire::proto::Call` without the
//! fields the player adds (`t`, `id`, `key`; WASM-014 Δ5):
//! `{"program":"NAME","args":[{"mode":"ref","json":…},…],"returning":true}`.
//! The answer is the player's frame of ABI A6/A8: `{"id":N,"result":R}` with
//! `R` the body of `proto::Reply` (`{"args":[json or null,…],"returning":json}`),
//! or `{"id":N,"error":{…}}` (ABI amendment A12).
//!
//! A call descriptor (constant, words): `n_args`, `returning` (0/1), then
//! `n_args × [mode (1 = BY REFERENCE), group descriptor, operand]`, then the
//! `RETURNING` group's descriptor and operand (0 when none).

use crate::fd::{Fd, K_ALNUM, K_NUM_DISPLAY};
use crate::json::{self, members, skip, string_char, Out, Text};
use crate::mem;
use crate::mv::Sender;
use crate::opd::{locate, parse, Loc};
use crate::state::{st, FRAME_SIZE, LAST_ERR_CLASS, LAST_ERR_MESSAGE, WS_FRAME, WS_FRAME_IN_LEN, WS_LAST_ERR};
use crate::val::Val;

/// What [`result`] answers.
pub const OK: i32 = 0;
/// The backend (or the player for it) answered with an error frame.
pub const FAILED: i32 = 1;
/// The answer does not fit its groups.
pub const BAD_ANSWER: i32 = 2;
/// The answer is not a frame of the ABI (or did not fit the workspace).
pub const BAD_FRAME: i32 = 3;

struct Call {
    d: u32,
}

impl Call {
    fn w(&self, i: u32) -> u32 {
        mem::rd_u32(self.d + 4 * i)
    }
    fn n_args(&self) -> u32 {
        self.w(0)
    }
    fn returning(&self) -> bool {
        self.w(1) != 0
    }
    /// `(by reference, descriptor, operand)` of argument `i`.
    fn arg(&self, i: u32) -> (bool, u32, u32) {
        let at = 2 + 3 * i;
        (self.w(at) != 0, self.w(at + 1), self.w(at + 2))
    }
    fn ret(&self) -> (u32, u32) {
        let at = 2 + 3 * self.n_args();
        (self.w(at), self.w(at + 1))
    }
}

/// Build the request frame of the call described at `call` to the program
/// named by operand `name`, and send it: the request id the host gave
/// (`>= 0`), or a negative code — the host's (ABI §9), or `-5` when the frame
/// does not fit, or `-(100 + e)` when encoding argument failed with mapping
/// error `e`.
pub fn request(base: u32, name: u32, call: u32) -> i32 {
    let c = Call { d: call };
    let at = st().ws + WS_FRAME;
    let mut o = Out::new(at, FRAME_SIZE);
    o.bytes(b"{\"program\":");
    // The name as the interpreter takes it: its text, trimmed, in upper case.
    let loc = locate(base, &parse(name));
    let b = mem::bytes(loc.addr, loc.len);
    let start = b.iter().position(|c| *c != b' ').unwrap_or(b.len());
    let end = b.iter().rposition(|c| *c != b' ').map_or(start, |p| p + 1);
    let mut up = [0u8; 160];
    let n = (end - start).min(up.len());
    for (k, &ch) in b[start..start + n].iter().enumerate() {
        up[k] = ch.to_ascii_uppercase();
    }
    o.string(&up[..n]);
    o.bytes(b",\"args\":[");
    for i in 0..c.n_args() {
        if i > 0 {
            o.byte(b',');
        }
        let (by_ref, desc, opd) = c.arg(i);
        o.bytes(if by_ref {
            b"{\"mode\":\"ref\",\"json\":"
        } else {
            b"{\"mode\":\"value\",\"json\":"
        });
        let addr = locate(base, &parse(opd)).addr;
        if let Err(e) = json::encode_into(desc, addr, &mut o) {
            note_error(b"VALIDATION", 4000, b"an argument of the call cannot travel as JSON");
            return -(100 + e);
        }
        o.byte(b'}');
    }
    o.bytes(if c.returning() {
        b"],\"returning\":true}"
    } else {
        b"],\"returning\":false}"
    });
    if o.full {
        note_error(b"LIMIT", 7000, b"the request is larger than the frame the module can send");
        return -5;
    }
    // SAFETY: the glue copies the frame out of memory 0.
    let id = unsafe { crate::host::remote(at, o.n) };
    if id < 0 {
        // The host could not send it (ABI §9): nothing reached the backend.
        note_error(b"TRANSPORT", 1000, b"the backend cannot be reached from this page");
    }
    id
}

// ── how the last call failed (`COBOL::"WS-LAST-ERROR"`) ────────────────────

fn note_error(class: &[u8], code: u32, message: &[u8]) {
    let at = st().ws + WS_LAST_ERR;
    let c = class.len().min(LAST_ERR_CLASS as usize);
    mem::wr_u32(at, c as u32);
    mem::bytes_mut(at + 4, c as u32).copy_from_slice(&class[..c]);
    mem::wr_u32(at + 4 + LAST_ERR_CLASS, code);
    let m = message.len().min(LAST_ERR_MESSAGE as usize);
    mem::wr_u32(at + 8 + LAST_ERR_CLASS, m as u32);
    mem::bytes_mut(at + 12 + LAST_ERR_CLASS, m as u32).copy_from_slice(&message[..m]);
}

fn clear_error() {
    note_error(b"", 0, b"");
}

/// The text of the JSON string at `i` as UTF-8, at most `cap` bytes.
fn string_bytes(t: &Text, i: u32, out: &mut [u8]) -> usize {
    let mut n = 0;
    let mut k = i + 1;
    while let Ok((Some(cp), next)) = string_char(t, k) {
        let mut buf = [0u8; 4];
        let s = char::from_u32(cp).unwrap_or('?').encode_utf8(&mut buf);
        if n + s.len() > out.len() {
            break;
        }
        out[n..n + s.len()].copy_from_slice(s.as_bytes());
        n += s.len();
        k = next;
    }
    n
}

/// Record the error frame's `{"class":…,"code":…,"message":…}` (ABI A6).
fn note_error_frame(t: &Text, err: u32) {
    let mut class = [0u8; LAST_ERR_CLASS as usize];
    let mut c = 0;
    if let Some(v) = member(t, err, b"class").filter(|v| t.get(*v) == Some(b'"')) {
        c = string_bytes(t, v, &mut class);
        class[..c].make_ascii_uppercase();
    }
    let mut code: u32 = 0;
    if let Some(mut v) = member(t, err, b"code") {
        while let Some(d @ b'0'..=b'9') = t.get(v) {
            code = code.saturating_mul(10).saturating_add((d - b'0') as u32);
            v += 1;
        }
    }
    let mut message = [0u8; LAST_ERR_MESSAGE as usize];
    let m = match member(t, err, b"message").filter(|v| t.get(*v) == Some(b'"')) {
        Some(v) => string_bytes(t, v, &mut message),
        None => 0,
    };
    note_error(&class[..c], code, &message[..m]);
}

/// `COBOL::"WS-LAST-ERROR"( class code message )`: how the last remote call
/// failed — spaces and zero after one that succeeded — as the interpreter
/// answers it (`remote.rs` `builtin_ws_last_error`). An operand of 0 is
/// skipped.
pub fn last_error(base: u32, class: u32, code: u32, message: u32) {
    let at = st().ws + WS_LAST_ERR;
    let text = |off: u32, len_at: u32| {
        let len = mem::rd_u32(at + len_at);
        Loc { addr: at + off, len, fd: Fd { kind: K_ALNUM, flags: 0, digits: 0, scale: 0, len, aux: 0 } }
    };
    if class != 0 {
        crate::mv::move_to(base, &Sender::Bytes(text(4, 0), false), &parse(class));
    }
    if code != 0 {
        let v = Val::int(mem::rd_u32(at + 4 + LAST_ERR_CLASS) as i64);
        let fd = Fd { kind: K_NUM_DISPLAY, flags: 0, digits: 10, scale: 0, len: 10, aux: 0 };
        crate::mv::move_to(base, &Sender::Num(v, fd), &parse(code));
    }
    if message != 0 {
        crate::mv::move_to(base, &Sender::Bytes(text(12 + LAST_ERR_CLASS, 8 + LAST_ERR_CLASS), false), &parse(message));
    }
}

/// Is the string at `i` the ASCII text `lit`?
fn key_is(t: &Text, mut i: u32, lit: &[u8]) -> bool {
    i += 1;
    for &want in lit {
        match string_char(t, i) {
            Ok((Some(cp), next)) if cp == want as u32 => i = next,
            _ => return false,
        }
    }
    matches!(string_char(t, i), Ok((None, _)))
}

/// The value of member `key` of the object at `obj`, if it has one.
fn member(t: &Text, obj: u32, key: &[u8]) -> Option<u32> {
    if t.get(obj) != Some(b'{') {
        return None;
    }
    let mut m = members(t, obj);
    while let Some((k, v)) = m.next_member(t) {
        if key_is(t, k, key) {
            return Some(v);
        }
    }
    None
}

/// Element `i` of the array at `arr`, if there is one.
fn element(t: &Text, arr: u32, i: u32) -> Option<u32> {
    if t.get(arr) != Some(b'[') {
        return None;
    }
    let mut m = members(t, arr);
    let mut k = 0;
    while let Some((_, v)) = m.next_member(t) {
        if k == i {
            return Some(v);
        }
        k += 1;
    }
    None
}

/// Read the answer `pr_resume` left in the workspace into the groups of the
/// call described at `call`: [`OK`], [`FAILED`], [`BAD_ANSWER`] or [`BAD_FRAME`].
/// Nothing is written unless everything fits.
pub fn result(base: u32, call: u32) -> i32 {
    let r = read_answer(base, call);
    match r {
        OK => clear_error(),
        BAD_ANSWER => note_error(b"VALIDATION", 4000, b"the answer does not fit its group"),
        BAD_FRAME => note_error(b"VALIDATION", 4001, b"the answer is not a frame of the ABI"),
        _ => {} // FAILED: the error frame was recorded as it was read
    }
    r
}

fn read_answer(base: u32, call: u32) -> i32 {
    let c = Call { d: call };
    let ws = st().ws;
    let len = mem::rd_u32(ws + WS_FRAME_IN_LEN);
    if len > FRAME_SIZE {
        return BAD_FRAME;
    }
    let t = Text {
        at: ws + WS_FRAME,
        len,
    };
    if json::check(&t).is_err() {
        return BAD_FRAME;
    }
    let top = t.ws(0);
    if t.get(top) != Some(b'{') {
        return BAD_FRAME;
    }
    if let Some(e) = member(&t, top, b"error") {
        note_error_frame(&t, e);
        return FAILED;
    }
    let Some(r) = member(&t, top, b"result") else {
        return BAD_FRAME;
    };
    // Every group the answer carries: `(descriptor, address, value)`.
    let mut todo: [(u32, u32, u32); 33] = [(0, 0, 0); 33];
    let mut n = 0usize;
    let args = member(&t, r, b"args");
    for i in 0..c.n_args().min(32) {
        let (by_ref, desc, opd) = c.arg(i);
        if !by_ref {
            continue;
        }
        if let Some(v) = args.and_then(|a| element(&t, a, i)) {
            if t.get(v) != Some(b'n') {
                todo[n] = (desc, locate(base, &parse(opd)).addr, v);
                n += 1;
            }
        }
    }
    if c.returning() {
        let (desc, opd) = c.ret();
        if let Some(v) = member(&t, r, b"returning") {
            if t.get(v) != Some(b'n') && desc != 0 {
                todo[n] = (desc, locate(base, &parse(opd)).addr, v);
                n += 1;
            }
        }
    }
    for commit in [false, true] {
        for &(desc, addr, v) in &todo[..n] {
            let end = skip(&t, v);
            if json::decode_with(desc, addr, t.at + v, end - v, commit) != 0 {
                return BAD_ANSWER;
            }
        }
    }
    OK
}
