// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Host commands and queries (ABI v1 §5–6): a control's property or method as
//! a path, written to the command batch.
//!
//! The generator builds a path with [`path_begin`], [`path_seg`] and
//! [`path_arg`], then ends it with one of [`set`], [`call`] or [`query`]. A
//! value crosses as **text**: a number as its decimal text, a character item
//! trimmed of the spaces around it — what the interpreter's
//! `val.as_display_string().trim()` sends the desktop host.
//!
//! ```text
//! path := varint inst, varint ctrl, nseg (one byte), seg*
//! seg  := varint name, nargs (one byte), text*          text := varint len, bytes
//! ```

use crate::disp;
use crate::host;
use crate::mem;
use crate::opd::*;
use crate::state::{err, st, ANSWER_SIZE, CTEMP_SIZE, NCTEMP, PATH_SIZE, WS_ANSWER, WS_CTEMP, WS_PATH};
use crate::val::Val;

pub const CMD_SET: u8 = 0x01;
pub const CMD_CALL: u8 = 0x02;
pub const Q_GET: u8 = 0x10;
pub const Q_CALLR: u8 = 0x11;
pub const Q_COUNT: u8 = 0x12;
/// ABI amendment A14: a property named by text, and an event raised.
pub const OP_SET_NAMED: u8 = 0x06;
pub const OP_RAISE: u8 = 0x07;

fn put(bytes: &[u8]) {
    let s = st();
    if s.path_len as usize + bytes.len() > PATH_SIZE as usize {
        crate::fail(err::SCRATCH_OVERFLOW);
    }
    mem::bytes_mut(s.ws + WS_PATH + s.path_len, bytes.len() as u32).copy_from_slice(bytes);
    s.path_len += bytes.len() as u32;
}

fn put_varint(v: u32) {
    let mut b = [0u8; 5];
    let n = disp::put_varint_pub(&mut b, v);
    put(&b[..n]);
}

/// Start a path on control `ctrl` of the instance the module is running.
pub fn path_begin(ctrl: u32) {
    st().path_len = 0;
    put_varint(st().inst);
    put_varint(ctrl);
    st().path_nseg_at = st().path_len;
    put(&[0]);
}

fn patch(at: u32, v: u8) {
    let s = st();
    mem::wr_u8(s.ws + WS_PATH + at, v);
}

fn bump(at: u32) {
    let s = st();
    let cur = mem::rd_u8(s.ws + WS_PATH + at);
    if cur >= 127 {
        crate::fail(err::SCRATCH_OVERFLOW);
    }
    patch(at, cur + 1);
}

/// Add a segment named by `name` (an index into the manifest's names).
pub fn path_seg(name: u32) {
    put_varint(name);
    st().path_nargs_at = st().path_len;
    put(&[0]);
    bump(st().path_nseg_at);
}

fn put_text(text: &[u8]) {
    put_varint(text.len() as u32);
    put(text);
}

/// The text a value crosses the boundary as (UTF-8, as the ABI requires:
/// invalid bytes become U+FFFD, the interpreter's lossy text of them).
fn text_of(base: u32, o: &Opd, num: &mut [u8; 64]) -> (u32, u32) {
    let (a, l) = raw_text_of(base, o, num);
    disp::lossy(a, l)
}

fn raw_text_of(base: u32, o: &Opd, num: &mut [u8; 64]) -> (u32, u32) {
    match o.tag {
        // A figurative constant is the one character it names, as the
        // interpreter's value of it: `SPACES` is a blank (so the text is empty
        // once trimmed), `ZEROS` and `NULL` the digit 0, `QUOTES` a quote,
        // `LOW-VALUES`/`HIGH-VALUES` their byte. `locate` points at the
        // operand's FIG code (SPACE = 1), which is not that character.
        T_FIG => {
            let ch = if o.fig == FIG_NULL { b'0' } else { fig_byte(o.fig) };
            let (at, _) = crate::mv::scratch();
            mem::wr_u8(at, ch);
            trimmed(at, 1)
        }
        T_LIT_ALNUM | T_ALL | T_CTEMP => {
            let l = locate(base, o);
            trimmed(l.addr, l.len)
        }
        T_LIT_NUM | T_TEMP => number_text(&value(base, o), num),
        _ => {
            let l = locate(base, o);
            if l.fd.is_numeric() {
                number_text(&crate::codec::load_value(&l), num)
            } else {
                trimmed(l.addr, l.len)
            }
        }
    }
}

fn number_text(v: &Val, num: &mut [u8; 64]) -> (u32, u32) {
    let n = match v {
        Val::Dec(d) => disp::format_dec(d, num),
        Val::F(f) => crate::float::format_f64(*f, num),
    };
    let (at, _) = crate::mv::scratch();
    mem::bytes_mut(at, n as u32).copy_from_slice(&num[..n]);
    (at, n as u32)
}

fn trimmed(addr: u32, len: u32) -> (u32, u32) {
    let b = mem::bytes(addr, len);
    let s = b.iter().position(|&x| x != b' ').unwrap_or(b.len());
    let e = b.iter().rposition(|&x| x != b' ').map_or(s, |i| i + 1).max(s);
    (addr + s as u32, (e - s) as u32)
}

/// Add argument `o` to the current segment.
pub fn path_arg(base: u32, o: u32) {
    let mut num = [0u8; 64];
    let (a, l) = text_of(base, &parse(o), &mut num);
    let t = mem::bytes(a, l);
    put_text(t);
    bump(st().path_nargs_at);
}

fn path_bytes() -> &'static [u8] {
    let s = st();
    mem::bytes(s.ws + WS_PATH, s.path_len)
}

/// `SET path value`.
pub fn set(base: u32, value_opd: u32) {
    let mut num = [0u8; 64];
    let (a, l) = text_of(base, &parse(value_opd), &mut num);
    let text = mem::bytes(a, l);
    let mut head = [0u8; 1];
    head[0] = CMD_SET;
    // The command is built in the batch directly: opcode, path, text.
    let mut tl = [0u8; 5];
    let n = disp::put_varint_pub(&mut tl, l);
    let total = 1 + st().path_len as usize + n + l as usize;
    if total > crate::state::BATCH_SIZE as usize {
        use crate::Part::{N, T};
        crate::fail_text(err::TEXT_TOO_LARGE, &[T(b"a property is set to "), N(l), T(b" bytes; one SET carries at most "), N(crate::state::BATCH_SIZE - total as u32 + l), T(b" bytes")]);
    }
    // One command must not be split by a flush.
    if (st().batch_len as usize) + total > crate::state::BATCH_SIZE as usize {
        disp::flush();
    }
    disp::batch_put(&head);
    disp::batch_put(path_bytes());
    disp::batch_put(&tl[..n]);
    disp::batch_put(text);
}

/// One command of opcode `op` naming control `ctrl` of the running instance
/// and two texts, the operands `a` and `b` (ABI amendment A14).
fn named_command(op: u8, base: u32, ctrl: u32, a: u32, b: u32) {
    let mut num = [0u8; 64];
    // The first text is copied out before the second is read: both may be
    // numbers, which are formatted in the same scratch area.
    let mut first = [0u8; 256];
    let (fa, fl) = text_of(base, &parse(a), &mut num);
    let fl = (fl as usize).min(first.len());
    first[..fl].copy_from_slice(&mem::bytes(fa, fl as u32)[..fl]);
    let (sa, sl) = text_of(base, &parse(b), &mut num);
    let second = mem::bytes(sa, sl);
    let mut head = [0u8; 21];
    head[0] = op;
    let mut n = 1;
    for v in [st().inst, ctrl, fl as u32] {
        n += disp::put_varint_pub(&mut head[n..], v);
    }
    let mut tl = [0u8; 5];
    let tn = disp::put_varint_pub(&mut tl, sl);
    let total = n + fl + tn + sl as usize;
    if total > crate::state::BATCH_SIZE as usize {
        crate::fail(err::SCRATCH_OVERFLOW);
    }
    if (st().batch_len as usize) + total > crate::state::BATCH_SIZE as usize {
        disp::flush();
    }
    disp::batch_put(&head[..n]);
    disp::batch_put(&first[..fl]);
    disp::batch_put(&tl[..tn]);
    disp::batch_put(second);
}

/// `SET_NAMED`: the property whose name operand `name` holds, of control
/// `ctrl`, gets the text of `value` (`COBOL::"SET-PROPERTY"` with the property
/// in a data item).
pub fn set_named(base: u32, ctrl: u32, name: u32, value: u32) {
    named_command(OP_SET_NAMED, base, ctrl, name, value);
}

/// `RAISE`: queue the event operand `event` names on control `ctrl`, with the
/// text of `arg` as its argument (`COBOL::"RAISE-EVENT"`).
pub fn raise(base: u32, ctrl: u32, event: u32, arg: u32) {
    named_command(OP_RAISE, base, ctrl, event, arg);
}

/// `CALL path` (the result, if any, is discarded).
pub fn call() {
    let total = 1 + st().path_len as usize;
    if (st().batch_len as usize) + total > crate::state::BATCH_SIZE as usize {
        disp::flush();
    }
    disp::batch_put(&[CMD_CALL]);
    disp::batch_put(path_bytes());
}

/// A query (`GET`, `CALLR`, `COUNT`) on the path: the commands staged so far
/// and the query go to the host together, and the answer text is left in
/// character temporary `k`, which `dynamic` marks as a property read.
pub fn query(op: u32, k: u32, dynamic: u32) {
    if k as usize >= NCTEMP {
        crate::fail(err::BAD_OPERAND);
    }
    let total = 1 + st().path_len as usize;
    if (st().batch_len as usize) + total > crate::state::BATCH_SIZE as usize {
        disp::flush();
    }
    disp::batch_put(&[op as u8]);
    disp::batch_put(path_bytes());
    let s = st();
    let len = s.batch_len;
    s.batch_len = 0;
    let ans = s.ws + WS_ANSWER;
    // SAFETY: the glue stages the batch, calls the host and writes the answer
    // text at `ans` (at most `ANSWER_SIZE` bytes); it returns the answer's
    // whole length, which may be more than it copied.
    let n = unsafe { host::query(s.ws + crate::state::WS_BATCH, len, ans, ANSWER_SIZE) };
    // The host refuses an answer longer than the arena's answer region (ABI
    // v1 §9 `-5`); one longer than this module's room would be copied short.
    // Either is an error the page shows: a statement never reads part of a
    // property as if it were all of it (the desktop hands it every byte).
    if n == -5 || (n > 0 && n as u32 > ANSWER_SIZE.min(CTEMP_SIZE)) {
        too_large_answer(if n > 0 { n as u32 } else { 0 });
    }
    if n < 0 {
        crate::fail(err::BAD_OPERAND);
    }
    let n = n as u32;
    let dst = s.ws + WS_CTEMP + k * CTEMP_SIZE;
    mem::copy(ans, dst, n);
    s.ctemp_len[k as usize] = n;
    if dynamic != 0 {
        s.ctemp_dynamic |= 1 << k;
    } else {
        s.ctemp_dynamic &= !(1 << k);
    }
}

/// Run-time error [`err::TEXT_TOO_LARGE`] for a query answer of `n` bytes (0:
/// the host only said it was over its limit).
#[cold]
fn too_large_answer(n: u32) -> ! {
    use crate::Part::{N, T};
    let max = ANSWER_SIZE.min(CTEMP_SIZE);
    if n > 0 {
        crate::fail_text(err::TEXT_TOO_LARGE, &[T(b"a property or method answered "), N(n), T(b" bytes; a statement can read at most "), N(max), T(b" bytes of one answer")])
    }
    crate::fail_text(err::TEXT_TOO_LARGE, &[T(b"a property or method answered more than the "), N(max), T(b" bytes a statement can read of one answer")])
}

// ── parking: WAIT-EVENT ─────────────────────────────────────────────────────

/// Remember, in the region slot `slot`, the two operand records that the next
/// event's names are written to (`WAIT-EVENT` event-id control-id).
pub fn park(base: u32, slot: u32, a: u32, b: u32) {
    mem::wr_u32(base + slot, a);
    mem::wr_u32(base + slot + 4, b);
}

/// Write the event's control name and event name into the items that
/// `WAIT-EVENT` named: padded with spaces, or cut, as a `MOVE` of the text would.
pub fn deliver(base: u32, slot: u32, ctrl_text: u32, ctrl_len: u32, ev_text: u32, ev_len: u32) {
    for (opd, text, len) in [(mem::rd_u32(base + slot), ev_text, ev_len), (mem::rd_u32(base + slot + 4), ctrl_text, ctrl_len)] {
        if opd == 0 {
            continue;
        }
        let loc = locate(base, &parse(opd));
        crate::mv::put_bytes(&loc, mem::bytes(text, len), false);
    }
}
